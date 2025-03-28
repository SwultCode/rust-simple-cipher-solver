#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use itertools::{structs, Itertools, Permutations};

use indicatif::ProgressBar;
use indicatif::ProgressStyle;

use std::collections::BinaryHeap;
use std::cmp::Reverse;
use std::sync::Mutex;
use std::sync::mpsc;
use rayon::prelude::*;

// Replace the existing COMMON_TRIGRAMS constant with this weighted version
// Format: (trigram, frequency weight)
const COMMON_TRIGRAMS: [(&str, usize); 10] = [
    ("the", 100), // Most common trigram
    ("and", 80),
    ("ing", 70),
    ("ent", 60),
    ("ion", 55),
    ("her", 50),
    ("for", 45),
    ("tha", 40),
    ("nth", 35),
    ("int", 30),
];

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([600.0, 340.0]),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            Ok(Box::<MyApp>::default())
        }),
    )
}

enum CipherType {
    Columnar,
}

struct MyApp {
    my_string: String,
    max_key_length: String,
    result_text: String,
    show_result: bool,
    factors: Option<Vec<usize>>,
    decryption_in_progress: bool,
    result_receiver: Option<mpsc::Receiver<String>>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            my_string: "".to_owned(),
            max_key_length: "8".to_owned(),
            result_text: "".to_owned(),
            show_result: false,
            factors: None,
            decryption_in_progress: false,
            result_receiver: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for results from the background thread
        if let Some(receiver) = &self.result_receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.result_text = result;
                    self.show_result = true;
                    self.decryption_in_progress = false;
                    self.result_receiver = None;
                },
                Err(mpsc::TryRecvError::Empty) => {},
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.decryption_in_progress = false;
                    self.result_receiver = None;
                }
            }
        }

        // Show results window when needed
        if self.show_result {
            egui::Window::new("Decryption Results")
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.label("Decrypted text:");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.result_text)
                                .desired_rows(15)
                                .desired_width(400.0),
                        );
                        if ui.button("Close").clicked() {
                            self.show_result = false;
                        }
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Decrypter");
            ui.vertical(|ui| {
                let string_label = ui.label("String to Decrypt: ");
                ui.add(
                    egui::TextEdit::multiline(&mut self.my_string)
                        .desired_rows(10)
                        .desired_width(400.),
                ).labelled_by(string_label.id);

                // Max key length input
                ui.horizontal(|ui| {
                    ui.label("Max Key Length: ");
                    ui.text_edit_singleline(&mut self.max_key_length);
                });

                // Show factors if available
                if let Some(factors) = &self.factors {
                    ui.horizontal(|ui| {
                        ui.label("Factors: ");
                        ui.label(format!("{:?}", factors));
                    });
                }

                ui.horizontal(|ui| {
                    if ui.button("Open File").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .pick_file()
                        {
                            match std::fs::read_to_string(&path) {
                                Ok(contents) => self.my_string = contents
                                    .chars()
                                    .filter(|c| !c.is_whitespace())
                                    .collect::<String>(),
                                Err(e) => println!("Error reading file: {}", e),
                            }
                        }
                    }

                    if ui.button("Get Factors").clicked() {
                        self.factors = Some(compute_factors(self.my_string.len()));
                    }

                    let decrypt_button = ui.add_enabled(
                        !self.decryption_in_progress,
                        egui::Button::new(
                            if self.decryption_in_progress { "Decrypting..." } else { "Decrypt" }
                        )
                    );

                    if decrypt_button.clicked() {
                        let max_key = self.max_key_length.parse::<usize>().unwrap_or(8);
                        let text_to_decrypt = self.my_string.clone();
                        let ctx_clone = ctx.clone();

                        // Create a channel for results
                        let (sender, receiver) = mpsc::channel();
                        self.result_receiver = Some(receiver);
                        self.decryption_in_progress = true;

                        // Start decryption in a separate thread
                        std::thread::spawn(move || {
                            let decrypter = Decrypter {
                                cipher_type: CipherType::Columnar,
                                key: None,
                                max_key_length: max_key,
                            };

                            let result = decrypter.decrypt(&text_to_decrypt);

                            // Send result back to UI thread
                            let _ = sender.send(result);
                            ctx_clone.request_repaint();
                        });
                    }
                });
            });
        });

        // Regular repaint when computation is in progress
        if self.decryption_in_progress {
            ctx.request_repaint();
        }
    }
}

struct Decrypter {
    cipher_type: CipherType,
    key: Option<String>,
    max_key_length: usize,
}

fn compute_factors(n: usize) -> Vec<usize> {
    let mut result: Vec<usize> = Vec::new();
    result.push(1);
    if n > 1 {
        result.push(n);
    }
    let int_sqrt = (n as f32).sqrt() as usize;
    for i in 2..=int_sqrt {
        if n % i == 0 {
            result.push(i);
            if i != n / i {
                result.push(n / i);
            }
        }
    }

    // Heuristic: sort the factors from smallest to biggest
    result.sort_by(|a, b| a.cmp(b));
    result
}

impl Decrypter {
    fn decrypt (&self, text: &str) -> String {
        match self.cipher_type {
            CipherType::Columnar => self.decrypt_columnar(text),
        }
    }

    fn decrypt_columnar(&self, text: &str) -> String {
        // Create a mutex-protected heap to collect results from different threads
        let heap = Mutex::new(BinaryHeap::new());

        // Find the factors of the text length
        let factors = compute_factors(text.len());
        println!("Factors: {:?}", factors);

        // Use the specified max key length instead of a constant
        let max_key_length = self.max_key_length;

        // Count total permutations first (for progress bar)
        let mut total_perms = 0;
        let mut permutation_counts = vec![0; max_key_length + 1];

        for key_length in 1..=max_key_length {
            let count = (0..key_length).permutations(key_length).count();
            permutation_counts[key_length] = count;
            total_perms += count;
        }

        // Setup progress bar
        let pb = ProgressBar::new(total_perms as u64);
        pb.set_style(ProgressStyle::with_template(
            "[{elapsed_precise}] [{wide_bar}] {pos}/{len} ({percent}%)",
        ).unwrap());

        let pb_ref = Mutex::new(pb);

        // Process key lengths in parallel
        (1..=max_key_length).into_par_iter().for_each(|key_length| {
            let permutations = (0..key_length).permutations(key_length);

            // For each permutation in this key length
            for permutation in permutations {
                let decrypted_text = self.columnar_inv(text, &permutation, false);
                let score = Self::english_score(&decrypted_text);

                // Update the heap with this candidate
                let mut heap_guard = heap.lock().unwrap();
                heap_guard.push(Reverse((score, decrypted_text, permutation)));

                if heap_guard.len() > 3 {  // Keep only top 3 candidates
                    heap_guard.pop();
                }

                // Update progress bar
                let mut pb_guard = pb_ref.lock().unwrap();
                pb_guard.inc(1);
            }
        });

        // Get reference to the progress bar to finish it
        let mut pb_guard = pb_ref.lock().unwrap();
        pb_guard.finish_with_message("Brute-force complete.");
        drop(pb_guard); // Explicitly drop the guard to release the lock

        // Extract results
        let heap_contents = heap.lock().unwrap();
        let mut best: Vec<_> = heap_contents.iter().cloned().collect();
        best.sort_by(|a, b| b.cmp(a)); // Sort in descending order
        drop(heap_contents); // Release the lock

        // Build a results string
        let mut result_output = String::new();
        for (i, Reverse((score, text, key))) in best.iter().enumerate() {
            let result_line = format!("{}. Score {}: Key: {:?}\n   Text: {}\n\n",
                                     i+1, score, key, text);
            result_output.push_str(&result_line);
        }

        // Return the formatted result string
        if !best.is_empty() {
            result_output
        } else {
            "No solution found.".to_owned()
        }
    }

    fn english_score(text: &str) -> usize {
        let text = text.to_lowercase();
        let mut score = 0;

        for (trigram, weight) in COMMON_TRIGRAMS {
            let count = text.matches(trigram).count();
            score += count * weight;
        }

        score
    }

    fn columnar_inv(&self, text: &str, key: &Vec<usize>, transpose: bool) -> String {
        // will be = key[(n mod (key.len))]
        let n = text.len();
        // text length
        let k_l = key.len();
        // |k|, key length
        let s_l = n / k_l;
        // |s|, number of letters in each column, rounded down, i.e. num rows
        let r = n % k_l;
        // this is the length of the last row, also num columns with an extra row

        let mut output = vec!['\0'; n];
        let chars: Vec<char> = text.chars().collect();

        // get the inverse key
        let mut key_inv = vec![0; k_l];
        for (i, &k) in key.iter().enumerate() {
            key_inv[k as usize] = i;
        }

        let mut offset = 0;
        for col in key_inv {
            let row_len = if col < r {s_l + 1} else {s_l};
            for row in 0..row_len {
                let c_index = offset + row;

                if transpose {
                    output[col * s_l + row + (if col < r { col } else {r})] = chars[c_index];
                } else {
                    output[row * k_l + col] = chars[c_index];
                }
            }
            offset += row_len;
        }

        output.into_iter().collect()
    }
}
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

// Common trigrams in English with their frequencies
const COMMON_TRIGRAMS: [(&str, usize); 20] = [
    ("the", 100),  // Most common trigram
    ("and", 80),
    ("ing", 70),
    ("ent", 60),
    ("ion", 55),
    ("her", 50),
    ("for", 45),
    ("tha", 40),
    ("nth", 35),
    ("int", 30),
    ("ere", 25),
    ("tio", 25),
    ("ter", 25),
    ("est", 25),
    ("ers", 25),
    ("ati", 25),
    ("hat", 25),
    ("ate", 25),
    ("all", 25),
    ("eth", 25),
];

// Common bigrams in English with their frequencies
const COMMON_BIGRAMS: [(&str, usize); 15] = [
    ("th", 100),  // Most common bigram
    ("he", 90),
    ("in", 80),
    ("er", 70),
    ("an", 60),
    ("re", 50),
    ("on", 45),
    ("at", 40),
    ("en", 35),
    ("nd", 30),
    ("ti", 30),
    ("es", 30),
    ("or", 30),
    ("te", 30),
    ("of", 30),
];

// Common English words with their frequencies
const COMMON_WORDS: [(&str, usize); 30] = [
    ("the", 300),  // Most common word
    ("be", 270),
    ("to", 240),
    ("of", 210),
    ("and", 180),
    ("a", 165),
    ("in", 150),
    ("that", 135),
    ("have", 120),
    ("i", 105),
    ("it", 90),
    ("for", 90),
    ("not", 90),
    ("on", 90),
    ("with", 90),
    ("he", 90),
    ("as", 90),
    ("you", 90),
    ("do", 90),
    ("at", 90),
    ("this", 90),
    ("but", 90),
    ("his", 90),
    ("by", 90),
    ("from", 90),
    ("they", 90),
    ("we", 90),
    ("say", 90),
    ("her", 90),
    ("she", 90),
];

// Character frequencies in English (in order of frequency)
const CHAR_FREQUENCIES: [(char, usize); 12] = [
    ('e', 100),  // Most common letter
    ('t', 90),
    ('a', 80),
    ('o', 75),
    ('i', 70),
    ('n', 65),
    ('s', 60),
    ('h', 55),
    ('r', 50),
    ('d', 45),
    ('l', 40),
    ('c', 35),
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

#[derive(PartialEq, Clone, Copy)]
enum CipherType {
    Columnar,
    Periodic,
}

struct MyApp {
    my_string: String,
    max_key_length: String,
    result_text: String,
    show_result: bool,
    factors: Option<Vec<usize>>,
    decryption_in_progress: bool,
    result_receiver: Option<mpsc::Receiver<String>>,
    transpose: bool,
    cipher_type: CipherType,
    period: String,
    check_all_periods: bool,
    selected_tab: usize,
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
            transpose: false,
            cipher_type: CipherType::Columnar,
            period: "3".to_owned(),
            check_all_periods: false,
            selected_tab: 0,
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
                    self.selected_tab = 0;
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
                .default_size([500.0, 300.0])
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.label("Top Decryption Candidates:");
                        ui.add_space(4.0);
                        
                        // Create tabs for each result
                        let results: Vec<&str> = self.result_text.split("\n\n").filter(|s| !s.is_empty()).collect();
                        
                        // Show tabs in the results window
                        ui.horizontal(|ui| {
                            for (i, result) in results.iter().enumerate() {
                                let parts: Vec<&str> = result.split('\n').collect();
                                if parts.len() >= 1 {
                                    let score = parts[0].split(": ").nth(1).unwrap_or("0");
                                    let tab_label = format!("Candidate {} (Score: {})", i + 1, score);
                                    if ui.selectable_label(i == self.selected_tab, tab_label).clicked() {
                                        self.selected_tab = i;
                                    }
                                }
                            }
                        });
                        ui.add_space(4.0);

                        // Show the selected result
                        if let Some(result) = results.get(self.selected_tab) {
                            ui.group(|ui| {
                                ui.vertical(|ui| {
                                    let parts: Vec<&str> = result.split('\n').collect();
                                    if parts.len() >= 2 {
                                        // Format the header (score and key)
                                        ui.horizontal(|ui| {
                                            ui.label(parts[0]);
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.button("Copy Key").clicked() {
                                                    if let Some(key_part) = parts[0].split("Key: ").nth(1) {
                                                        ui.output_mut(|o| o.copied_text = key_part.to_string());
                                                    }
                                                }
                                                ui.label("🔑");
                                                
                                                if ui.button("Copy Text").clicked() {
                                                    let text = parts[1].trim_start_matches("   Text: ");
                                                    ui.output_mut(|o| o.copied_text = text.to_string());
                                                }
                                                ui.label("📋");
                                            });
                                        });
                                        
                                        // Format the decrypted text
                                        ui.add_space(2.0);
                                        ui.add(
                                            egui::TextEdit::multiline(&mut parts[1].trim_start_matches("   Text: ").to_string())
                                                .desired_rows(2)
                                                .desired_width(400.0)
                                                .interactive(false)
                                        );
                                    }
                                });
                            });
                        }
                        
                        ui.add_space(4.0);
                        if ui.button("Close").clicked() {
                            self.show_result = false;
                            self.selected_tab = 0;
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
                        .desired_rows(5)
                        .desired_width(400.),
                ).labelled_by(string_label.id);

                ui.horizontal(|ui| {
                    // Method selection
                    ui.label("Method: ");
                    egui::ComboBox::from_label("")
                        .selected_text(match self.cipher_type {
                            CipherType::Columnar => "Columnar Transposition",
                            CipherType::Periodic => "Periodic Transposition",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.cipher_type, CipherType::Columnar, "Columnar Transposition");
                            ui.selectable_value(&mut self.cipher_type, CipherType::Periodic, "Periodic Transposition");
                        });

                    // Transpose checkbox
                    ui.checkbox(&mut self.transpose, "Transpose");
                });

                // Settings based on method
                match self.cipher_type {
                    CipherType::Columnar => {
                        ui.horizontal(|ui| {
                            ui.label("Max Key Length: ");
                            ui.text_edit_singleline(&mut self.max_key_length);
                        });
                    },
                    CipherType::Periodic => {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Period: ");
                                ui.text_edit_singleline(&mut self.period);
                                ui.checkbox(&mut self.check_all_periods, "Check all periods up to");
                            });
                            if self.check_all_periods {
                                ui.horizontal(|ui| {
                                    ui.label("Max Period: ");
                                    ui.text_edit_singleline(&mut self.max_key_length);
                                });
                            }
                        });
                    }
                }

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
                        let text_to_decrypt = self.my_string.clone();
                        let ctx_clone = ctx.clone();
                        let transpose = self.transpose;
                        let cipher_type = self.cipher_type;
                        let max_key = self.max_key_length.parse::<usize>().unwrap_or(8);
                        let period = self.period.parse::<usize>().unwrap_or(3);
                        let check_all_periods = self.check_all_periods;

                        // Create a channel for results
                        let (sender, receiver) = mpsc::channel();
                        self.result_receiver = Some(receiver);
                        self.decryption_in_progress = true;

                        // Start decryption in a separate thread
                        std::thread::spawn(move || {
                            let decrypter = Decrypter {
                                cipher_type,
                                key: None,
                                max_key_length: max_key,
                                period,
                                check_all_periods,
                            };

                            let result = decrypter.decrypt_with_transpose(&text_to_decrypt, transpose);

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
    period: usize,
    check_all_periods: bool,
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
            CipherType::Periodic => self.decrypt_periodic(text),
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
            }
        });

        // Extract results
        let heap_contents = heap.lock().unwrap();
        let mut best: Vec<_> = heap_contents.iter().cloned().collect();
        best.sort_by(|a, b| a.cmp(b)); // Sort in ascending order (highest scores first)
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

    fn decrypt_periodic(&self, text: &str) -> String {
        // Create a mutex-protected heap to collect results from different threads
        let heap = Mutex::new(BinaryHeap::new());
        let period = self.period;
        let max_period = if self.check_all_periods { self.max_key_length } else { period };
        let periods_to_check = if self.check_all_periods {
            (period..=max_period).collect::<Vec<usize>>()
        } else {
            vec![period]
        };

        // Process each period in parallel
        periods_to_check.into_par_iter().for_each(|current_period| {
            let permutations: Vec<Vec<usize>> = (0..current_period).permutations(current_period).collect();

            // Process permutations for this period
            for permutation in permutations {
                let decrypted_text = self.periodic_inv(text, &permutation);
                let score = Self::english_score(&decrypted_text);

                // Update the heap with this candidate
                let mut heap_guard = heap.lock().unwrap();
                heap_guard.push(Reverse((score, decrypted_text, permutation)));

                if heap_guard.len() > 3 {  // Keep only top 3 candidates
                    heap_guard.pop();
                }
            }
        });

        // Extract results
        let heap_contents = heap.lock().unwrap();
        let mut best: Vec<_> = heap_contents.iter().cloned().collect();
        best.sort_by(|a, b| a.cmp(b)); // Sort in ascending order (highest scores first)
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

        // Score based on trigrams
        for (trigram, weight) in COMMON_TRIGRAMS {
            let count = text.matches(trigram).count();
            score += count * weight;
        }

        // Score based on bigrams
        for (bigram, weight) in COMMON_BIGRAMS {
            let count = text.matches(bigram).count();
            score += count * weight;
        }

        // Score based on common words
        for (word, weight) in COMMON_WORDS {
            let count = text.matches(word).count();
            score += count * weight;
        }

        // Score based on character frequencies
        let text_chars: Vec<char> = text.chars().collect();
        for (c, weight) in CHAR_FREQUENCIES {
            let count = text_chars.iter().filter(|&&x| x == c).count();
            score += count * weight;
        }

        // Bonus for spaces (helps identify word boundaries)
        let space_count = text.matches(' ').count();
        score += space_count * 20;

        // Penalty for non-alphabetic characters (except spaces)
        let non_alpha_count = text.chars().filter(|c| !c.is_alphabetic() && !c.is_whitespace()).count();
        score -= non_alpha_count * 30;

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

    fn decrypt_with_transpose(&self, text: &str, transpose: bool) -> String {
        match self.cipher_type {
            CipherType::Columnar => self.decrypt_columnar_with_transpose(text, transpose),
            CipherType::Periodic => self.decrypt_periodic(text),
        }
    }

    fn decrypt_columnar_with_transpose(&self, text: &str, transpose: bool) -> String {
        // Create a mutex-protected heap to collect results from different threads
        let heap = Mutex::new(BinaryHeap::new());

        // Find the factors of the text length
        let factors = compute_factors(text.len());
        println!("Factors: {:?}", factors);

        // Use the specified max key length instead of a constant
        let max_key_length = self.max_key_length;

        // Process key lengths in parallel
        (1..=max_key_length).into_par_iter().for_each(|key_length| {
            let permutations = (0..key_length).permutations(key_length);

            // For each permutation in this key length
            for permutation in permutations {
                let decrypted_text = self.columnar_inv(text, &permutation, transpose);
                let score = Self::english_score(&decrypted_text);

                // Update the heap with this candidate
                let mut heap_guard = heap.lock().unwrap();
                heap_guard.push(Reverse((score, decrypted_text, permutation)));

                if heap_guard.len() > 3 {  // Keep only top 3 candidates
                    heap_guard.pop();
                }
            }
        });

        // Extract results
        let heap_contents = heap.lock().unwrap();
        let mut best: Vec<_> = heap_contents.iter().cloned().collect();
        best.sort_by(|a, b| a.cmp(b)); // Sort in ascending order (highest scores first)
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

    fn periodic_inv(&self, text: &str, key: &Vec<usize>) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut result = Vec::new();
        let period = key.len();

        // Process the text in chunks of size period
        for chunk in chars.chunks(period) {
            if chunk.len() == period {
                // Only apply permutation to complete chunks
                let mut chunk_vec = chunk.to_vec();
                // Apply the inverse permutation
                for (i, &pos) in key.iter().enumerate() {
                    chunk_vec[pos] = chunk[i];
                }
                result.extend(chunk_vec);
            } else {
                // For partial chunks at the end, just add them as is
                result.extend(chunk);
            }
        }

        result.into_iter().collect()
    }
}
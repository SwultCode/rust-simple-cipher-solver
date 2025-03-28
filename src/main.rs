#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use itertools::{structs, Itertools, Permutations};

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
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            my_string: "".to_owned(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Decrypter");
            ui.vertical(|ui| {
                let string_label = ui.label("String to Decrypt: ");
                ui.add(
                    egui::TextEdit::multiline(&mut self.my_string)
                        .desired_rows(10)
                        .desired_width(f32::INFINITY),
                ).labelled_by(string_label.id);

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

                    if ui.button("Decrypt").clicked() {
                        let decrypter = Decrypter {
                            cipher_type: CipherType::Columnar,
                            key: None,
                        };

                        let result = decrypter.decrypt(&self.my_string);

                        println!("{}", result);
                    }

                });

            });
        });
    }
}

struct Decrypter {
    cipher_type: CipherType,
    key: Option<String>,
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
        // Find the factors of the text length
        let factors = compute_factors(text.len());
        println!("Factors: {:?}", factors);

        // Constants for configuration
        const MAX_KEY_LENGTH: usize = 7;
        const ENGLISH_PATTERN: &str = "th";
        const MIN_PATTERN_OCCURRENCES: usize = 40;

        // Try different key lengths and their permutations
        for key_length in 1..=MAX_KEY_LENGTH {
            let permutations = (0..key_length).permutations(key_length);

            for permutation in permutations {
                let decrypted_text = self.columnar_inv(text, &permutation);

                // Check if the decrypted text resembles English
                if self.is_likely_english(&decrypted_text, ENGLISH_PATTERN, MIN_PATTERN_OCCURRENCES) {
                    println!("Possible decryption found: {}", decrypted_text);
                }
            }
        }

        // Return the original text if no better solution is found
        text.to_owned()
    }

    /// Checks if text resembles English based on pattern occurrence frequency
    fn is_likely_english(text: &str, pattern: &str, min_occurrences: usize) -> bool {
        text.matches(pattern).count() >= min_occurrences
    }

    fn columnar_inv(&self, text: &str, key: &Vec<usize>) -> String {
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
                output[row * k_l + col] = chars[c_index];
            }
            offset += row_len;
        }

        output.into_iter().collect()
    }
}
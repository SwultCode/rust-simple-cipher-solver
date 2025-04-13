#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use itertools::{structs, Itertools, Permutations};

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

// Number of top letters to check for each position in Beaufort cipher
const BEAUFORT_TOP_LETTERS: usize = 2;

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
    Vigenere,
    Beaufort,
}

#[derive(Clone)]
struct Candidate {
    name: String,
    score: f32,
    text: String,
}

struct MyApp {
    my_string: String,
    max_key_length: String,
    result_text: String,
    show_result: bool,
    factors: Option<Vec<usize>>,
    decryption_in_progress: bool,
    result_receiver: Option<mpsc::Receiver<Vec<Candidate>>>,
    transpose: bool,
    cipher_type: CipherType,
    period: String,
    check_all_periods: bool,
    selected_tab: usize,
    max_ic_period: f32,
    candidates: Vec<Candidate>,
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
            max_ic_period: 10.0,
            candidates: Vec::new(),
        }
    }
}

impl MyApp {
    fn show_candidates_dialog(&mut self, ctx: &egui::Context, title: &str) {
        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .default_size([500.0, 300.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label("Top Candidates:");
                    ui.add_space(4.0);
                    
                    // Show tabs in the results window
                    ui.horizontal(|ui| {
                        for (i, candidate) in self.candidates.iter().enumerate() {
                            let tab_label = format!("{} (Score: {:.3})", candidate.name, candidate.score);
                            if ui.selectable_label(i == self.selected_tab, tab_label).clicked() {
                                self.selected_tab = i;
                            }
                        }
                    });
                    ui.add_space(4.0);

                    // Show the selected result
                    if let Some(candidate) = self.candidates.get(self.selected_tab) {
                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                // Format the header (score and name)
                                ui.horizontal(|ui| {
                                    ui.label(format!("Score: {:.3}", candidate.score));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Copy Text").clicked() {
                                            ui.output_mut(|o| o.copied_text = candidate.text.clone());
                                        }
                                        ui.label("📋");
                                    });
                                });
                                
                                // Format the text
                                ui.add_space(2.0);
                                ui.add(
                                    egui::TextEdit::multiline(&mut candidate.text.clone())
                                        .desired_rows(2)
                                        .desired_width(f32::INFINITY)
                                        .interactive(false)
                                );
                            });
                        });
                    }
                    
                    ui.add_space(4.0);
                    if ui.button("Close").clicked() {
                        self.show_result = false;
                        self.selected_tab = 0;
                        self.candidates.clear();
                    }
                });
            });
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for results from the background thread
        if let Some(receiver) = &self.result_receiver {
            match receiver.try_recv() {
                Ok(candidates) => {
                    self.candidates = candidates;
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
            self.show_candidates_dialog(ctx, "Decryption Results");
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                // Title with better styling
                ui.add_space(8.0);
                ui.heading("Cipher Decrypter");
                ui.add_space(16.0);

                // Input section
                ui.group(|ui| {
                    ui.label("Input Text:");
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::multiline(&mut self.my_string)
                            .desired_rows(5)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace)
                    );
                });
                ui.add_space(8.0);

                // Settings section
                ui.group(|ui| {
                    ui.label("Decryption Settings:");
                    ui.add_space(8.0);

                    // Method selection with better layout
                    ui.horizontal(|ui| {
                        ui.label("Method:");
                        ui.add_space(8.0);
                        egui::ComboBox::from_label("")
                            .selected_text(match self.cipher_type {
                                CipherType::Columnar => "Columnar Transposition",
                                CipherType::Periodic => "Periodic Transposition",
                                CipherType::Vigenere => "Vigenère Cipher",
                                CipherType::Beaufort => "Beaufort Cipher",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.cipher_type, CipherType::Columnar, "Columnar Transposition");
                                ui.selectable_value(&mut self.cipher_type, CipherType::Periodic, "Periodic Transposition");
                                ui.selectable_value(&mut self.cipher_type, CipherType::Vigenere, "Vigenère Cipher");
                                ui.selectable_value(&mut self.cipher_type, CipherType::Beaufort, "Beaufort Cipher");
                            });

                        // Transpose checkbox (only for transposition ciphers)
                        if matches!(self.cipher_type, CipherType::Columnar | CipherType::Periodic) {
                            ui.add_space(16.0);
                            ui.checkbox(&mut self.transpose, "Transpose");
                        }
                    });
                    ui.add_space(8.0);

                    // Settings based on method
                    match self.cipher_type {
                        CipherType::Columnar => {
                            ui.horizontal(|ui| {
                                ui.label("Max Key Length:");
                                ui.add_space(8.0);
                                ui.add(egui::TextEdit::singleline(&mut self.max_key_length)
                                    .desired_width(60.0));
                            });
                        },
                        CipherType::Periodic => {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Period:");
                                    ui.add_space(8.0);
                                    ui.add(egui::TextEdit::singleline(&mut self.period)
                                        .desired_width(60.0));
                                    ui.add_space(16.0);
                                    ui.checkbox(&mut self.check_all_periods, "Check all periods up to");
                                });
                                if self.check_all_periods {
                                    ui.horizontal(|ui| {
                                        ui.label("Max Period:");
                                        ui.add_space(8.0);
                                        ui.add(egui::TextEdit::singleline(&mut self.max_key_length)
                                            .desired_width(60.0));
                                    });
                                }
                            });
                        }
                        CipherType::Vigenere => {
                            ui.horizontal(|ui| {
                                ui.label("Period:");
                                ui.add_space(8.0);
                                ui.add(egui::TextEdit::singleline(&mut self.period)
                                    .desired_width(60.0));
                            });
                        },
                        CipherType::Beaufort => {
                            ui.horizontal(|ui| {
                                ui.label("Period:");
                                ui.add_space(8.0);
                                ui.add(egui::TextEdit::singleline(&mut self.period)
                                    .desired_width(60.0));
                            });
                        }
                    }

                    // Show factors if available
                    if let Some(factors) = &self.factors {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label("Factors:");
                            ui.add_space(8.0);
                            ui.label(format!("{:?}", factors));
                        });
                    }
                });
                ui.add_space(8.0);

                // IC Analysis section
                ui.group(|ui| {
                    ui.label("Index of Coincidence Analysis:");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Max Period:");
                        ui.add_space(8.0);
                        ui.add(egui::Slider::new(&mut self.max_ic_period, 1.0..=20.0).step_by(1.0));
                        ui.label(format!("{:.0}", self.max_ic_period));
                    });
                });
                ui.add_space(8.0);

                // Action buttons
                ui.horizontal(|ui| {
                    if ui.button("📂 Open File").clicked() {
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

                    if ui.button("🔢 Get Factors").clicked() {
                        self.factors = Some(compute_factors(self.my_string.len()));
                    }

                    if ui.button("📊 Find IC").clicked() {
                        let text = self.my_string.clone();
                        let max_period = self.max_ic_period as usize;
                        let mut candidates = Vec::new();
                        
                        // Create overview tab first
                        let mut overview_text = String::new();
                        for period in 1..=max_period {
                            let output = Decrypter::index_of_coincidence(&text, period);
                            let avg = output.iter().sum::<f32>() / output.len() as f32;
                            let score = (avg - 0.066).abs(); // Closer to English IC is better
                            
                            overview_text.push_str(&format!("p={}: {:.3} | [{}]\n", period, (avg - 0.066).abs(),
                                output.iter().map(|&x| format!("{:.3}", x)).collect::<Vec<String>>().join(", ")));
                            
                            candidates.push(Candidate {
                                name: format!("p={}", period),
                                score,
                                text: format!("IC: {:.3}\nValues: [{}]", avg, 
                                    output.iter().map(|&x| format!("{:.3}", x)).collect::<Vec<String>>().join(", ")),
                            });
                        }
                        
                        // Sort by score (closest to English IC first)
                        candidates.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
                        
                        // Add overview as first candidate
                        candidates.insert(0, Candidate {
                            name: "Overview".to_string(),
                            score: 0.0,
                            text: overview_text,
                        });
                        
                        // Keep only top 4 candidates after overview
                        if candidates.len() > 5 {
                            candidates.truncate(5);
                        }
                        
                        self.candidates = candidates;
                        self.show_result = true;
                        self.selected_tab = 0;
                    }

                    let decrypt_button = ui.add_enabled(
                        !self.decryption_in_progress,
                        egui::Button::new(
                            if self.decryption_in_progress { "🔍 Decrypting..." } else { "🔍 Decrypt" }
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
    fn decrypt(&self, text: &str) -> Vec<Candidate> {
        match self.cipher_type {
            CipherType::Columnar => self.decrypt_columnar(text),
            CipherType::Periodic => self.decrypt_periodic(text),
            CipherType::Vigenere => self.decrypt_vigenere(text),
            CipherType::Beaufort => self.decrypt_beaufort(text),
        }
    }

    fn decrypt_columnar(&self, text: &str) -> Vec<Candidate> {
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

        // Convert to candidates
        best.iter().enumerate().map(|(i, Reverse((score, text, key)))| {
            Candidate {
                name: format!("Candidate {}", i + 1),
                score: *score as f32,
                text: format!("Key: {:?}\nText: {}", key, text),
            }
        }).collect()
    }

    fn decrypt_periodic(&self, text: &str) -> Vec<Candidate> {
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

        // Convert to candidates
        best.iter().enumerate().map(|(i, Reverse((score, text, key)))| {
            Candidate {
                name: format!("Candidate {}", i + 1),
                score: *score as f32,
                text: format!("Key: {:?}\nText: {}", key, text),
            }
        }).collect()
    }

    fn decrypt_vigenere(&self, text: &str) -> Vec<Candidate> {
        let period = self.period;
        println!("Starting Vigenère decryption with period {}", period);
        
        // Split text into period components
        let mut char_groups: Vec<Vec<char>> = vec![Vec::new(); period];
        for (i, c) in text.chars().enumerate() {
            char_groups[i % period].push(c);
        }

        // For each position in the key, find the most likely shifts
        let mut key_positions: Vec<Vec<usize>> = Vec::new();
        
        // For each position in the key
        for (i, group) in char_groups.iter().enumerate() {
            println!("Analyzing position {} ({} characters)", i, group.len());
            
            // Count frequencies in this group
            let mut freq_table = vec![0; 26];
            for c in group {
                if c.is_ascii_alphabetic() {
                    freq_table[c.to_ascii_lowercase() as usize - 'a' as usize] += 1;
                }
            }

            // Find top 3 most common letters in this group
            let mut freq_positions: Vec<(usize, usize)> = freq_table.iter()
                .enumerate()
                .map(|(pos, &count)| (pos, count))
                .collect();
            
            freq_positions.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
            let top_3_positions: Vec<usize> = freq_positions.iter()
                .take(3)
                .map(|&(pos, _)| pos)
                .collect();

            println!("Top 3 letters at position {}: {:?}", i, 
                top_3_positions.iter().map(|&p| ((p as u8 + b'a') as char)).collect::<Vec<char>>());

            // For each of the top 3 positions, calculate the shift assuming it maps to 'E'
            let mut shifts = Vec::new();
            for &pos in &top_3_positions {
                let shift = (pos + 22) % 26; // 22 = (26 - 4) mod 26
                shifts.push(shift);
            }
            key_positions.push(shifts);
        }

        // Generate all possible combinations of shifts
        let mut key_candidates: Vec<Vec<usize>> = Vec::new();
        for shifts in key_positions {
            if key_candidates.is_empty() {
                for &shift in &shifts {
                    key_candidates.push(vec![shift]);
                }
            } else {
                let mut new_candidates = Vec::new();
                for mut key in key_candidates {
                    for &shift in &shifts {
                        let mut new_key = key.clone();
                        new_key.push(shift);
                        new_candidates.push(new_key);
                    }
                }
                key_candidates = new_candidates;
            }
        }

        println!("Generated {} key candidates", key_candidates.len());

        // Try each key candidate and score the results
        let mut scored_results: Vec<(String, String, f32)> = Vec::new();
        for (i, key) in key_candidates.iter().enumerate() {
            if i % 100 == 0 {
                println!("Testing key candidate {}/{}", i, key_candidates.len());
            }
            
            if key.len() == period {
                let mut result = String::new();
                for (i, c) in text.chars().enumerate() {
                    if c.is_ascii_alphabetic() {
                        let shift = key[i % period];
                        let base = if c.is_uppercase() { 'A' } else { 'a' } as u8;
                        let decrypted = ((c as u8 - base + 26 - shift as u8) % 26 + base) as char;
                        result.push(decrypted);
                    } else {
                        result.push(c);
                    }
                }

                let score = Self::english_score(&result) as f32;
                let key_str: String = key.iter()
                    .map(|&shift| ((shift as u8 + b'a') as char))
                    .collect();
                
                scored_results.push((key_str, result, score));
            }
        }

        println!("Found {} valid results", scored_results.len());

        // Sort by score and take top 5
        scored_results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        scored_results.truncate(5);

        // Convert to candidates
        scored_results.iter().enumerate().map(|(i, (key, text, score))| {
            Candidate {
                name: format!("Candidate {}", i + 1),
                score: *score,
                text: format!("Key: {}\nDecryption:\n{}", key, text),
            }
        }).collect()
    }

    fn decrypt_beaufort(&self, text: &str) -> Vec<Candidate> {
        let period = self.period;
        println!("Starting Beaufort decryption with period {}", period);
        
        // Split text into period components
        let mut char_groups: Vec<Vec<char>> = vec![Vec::new(); period];
        for (i, c) in text.chars().enumerate() {
            char_groups[i % period].push(c);
        }

        // For each position in the key, find the most likely shifts
        let mut key_positions: Vec<Vec<usize>> = Vec::new();
        
        // For each position in the key
        for (i, group) in char_groups.iter().enumerate() {
            println!("Analyzing position {} ({} characters)", i, group.len());
            
            // Count frequencies in this group
            let mut freq_table = vec![0; 26];
            for c in group {
                if c.is_ascii_alphabetic() {
                    freq_table[c.to_ascii_lowercase() as usize - 'a' as usize] += 1;
                }
            }

            // Find top N most common letters in this group
            let mut freq_positions: Vec<(usize, usize)> = freq_table.iter()
                .enumerate()
                .map(|(pos, &count)| (pos, count))
                .collect();
            
            freq_positions.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
            let top_positions: Vec<usize> = freq_positions.iter()
                .take(BEAUFORT_TOP_LETTERS)
                .map(|&(pos, _)| pos)
                .collect();

            println!("Top {} letters at position {}: {:?}", BEAUFORT_TOP_LETTERS, i, 
                top_positions.iter().map(|&p| ((p as u8 + b'a') as char)).collect::<Vec<char>>());

            // For each of the top positions, calculate the shift assuming it maps to 'E'
            let mut shifts = Vec::new();
            for &pos in &top_positions {
                let shift = (pos + 4) % 26;
                shifts.push(shift);
            }
            key_positions.push(shifts);
        }

        // Generate all possible combinations of shifts
        let mut key_candidates: Vec<Vec<usize>> = Vec::new();
        for shifts in key_positions {
            if key_candidates.is_empty() {
                for &shift in &shifts {
                    key_candidates.push(vec![shift]);
                }
            } else {
                let mut new_candidates = Vec::new();
                for mut key in key_candidates {
                    for &shift in &shifts {
                        let mut new_key = key.clone();
                        new_key.push(shift);
                        new_candidates.push(new_key);
                    }
                }
                key_candidates = new_candidates;
            }
        }

        println!("Generated {} key candidates", key_candidates.len());

        // Try each key candidate and score the results
        let mut scored_results: Vec<(String, String, f32)> = Vec::new();
        for (i, key) in key_candidates.iter().enumerate() {
            if i % 100 == 0 {
                println!("Testing key candidate {}/{}", i, key_candidates.len());
            }
            
            if key.len() == period {
                let mut result = String::new();
                for (i, c) in text.chars().enumerate() {
                    if c.is_ascii_alphabetic() {
                        let shift = key[i % period];
                        let base = if c.is_uppercase() { 'A' } else { 'a' } as u8;
                        let decrypted = ((shift as u8 + 26 - (c as u8 - base)) % 26 + base) as char;
                        result.push(decrypted);
                    } else {
                        result.push(c);
                    }
                }

                let score = Self::english_score(&result) as f32;
                let key_str: String = key.iter()
                    .map(|&shift| ((shift as u8 + b'a') as char))
                    .collect();
                
                scored_results.push((key_str, result, score));
            }
        }

        println!("Found {} valid results", scored_results.len());

        // Sort by score and take top 5
        scored_results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        scored_results.truncate(5);

        // Convert to candidates
        scored_results.iter().enumerate().map(|(i, (key, text, score))| {
            Candidate {
                name: format!("Candidate {}", i + 1),
                score: *score,
                text: format!("Key: {}\nDecryption:\n{}", key, text),
            }
        }).collect()
    }

    fn decrypt_with_transpose(&self, text: &str, transpose: bool) -> Vec<Candidate> {
        match self.cipher_type {
            CipherType::Columnar => self.decrypt_columnar_with_transpose(text, transpose),
            CipherType::Periodic => self.decrypt_periodic(text),
            CipherType::Vigenere => self.decrypt_vigenere(text),
            CipherType::Beaufort => self.decrypt_beaufort(text),
        }
    }

    fn decrypt_columnar_with_transpose(&self, text: &str, transpose: bool) -> Vec<Candidate> {
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

        // Convert to candidates
        best.iter().enumerate().map(|(i, Reverse((score, text, key)))| {
            Candidate {
                name: format!("Candidate {}", i + 1),
                score: *score as f32,
                text: format!("Key: {:?}\nText: {}", key, text),
            }
        }).collect()
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

        score
    }

    fn print_index_of_coincidence_table(text: &str) {
        for period in 1..=10 {
            let output = Self::index_of_coincidence(text, period);
            let avg = output.iter().sum::<f32>() / output.len() as f32;
            println!("p={}: {:?} | {:?}", period, (avg - 0.066).abs(), output);
        }
    }

    fn index_of_coincidence(text: &str, period: usize) -> Vec<f32> {
        // 1. split text into d period components
        // 0 1 2 3 4 5 6 7 8 9 10
        // 0 1 2 0 1 2 0 1 2 0 1

        let mut char_groups: Vec<Vec<char>> = vec![Vec::new(); period];
        let mut output: Vec<f32> = vec![0.0; period];

        for (i, char) in text.chars().enumerate() {
            char_groups[i % period].push(char);
        }

        for (i, char_group) in char_groups.into_iter().enumerate() {
            let N = char_group.len();
            // 2. get frequency table for the char_group
            let mut freq_table: Vec<f32> = vec![0.0; 26];
            for char in char_group {
                freq_table[char as usize - 'a' as usize] += 1.0;
            }

            //3.
            // output[i % period] +=
            //     Decrypter::get_frequency(char) * (Decrypter::get_frequency(char) - 1.0) /
            //     (period as f32 * (period as f32 - 1.));
            for j in 0..26 {
                output[i % period] += freq_table[j] * (freq_table[j] - 1.0) / (N as f32 * (N as f32 - 1.));
            }
        }

        //println!("Char groups: {:?}", output);

        output
    }
}
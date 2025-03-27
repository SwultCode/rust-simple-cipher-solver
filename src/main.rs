#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use egui_file::FileDialog;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
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
                ui.text_edit_multiline(&mut self.my_string)
                    .labelled_by(string_label.id);

                ui.horizontal(|ui| {

                    if ui.button("Open File").clicked() {
                        // ui
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

impl Decrypter {
    fn decrypt (&self, text: &str) -> String {
        match self.cipher_type {
            CipherType::Columnar => self.decrypt_columnar(text),
        }
    }

    fn decrypt_columnar(&self, text: &str) -> String {
        todo!()
    }
}
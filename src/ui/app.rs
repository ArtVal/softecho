//! Клиентский UI (egui). Общается с движком только через Command / геттеры / tick.

use crate::engine::{
    AsrStatus, CheckResult, Command, Engine, Exercise, ModelDownloadState, Screen, UserAnswer,
};
use crate::ui::theme::apply_theme;
use crate::ui::widgets::{big_button, str_byte_tail};

use eframe::egui::{self, Color32, FontId, RichText};

pub struct UiApp {
    engine: Engine,
}

impl UiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        Self {
            engine: Engine::new(),
        }
    }
}

impl eframe::App for UiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let tick = self.engine.tick();
        if tick.want_repaint {
            ctx.request_repaint();
        } else if let Some(after) = tick.repaint_after {
            ctx.request_repaint_after(after);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            match self.engine.screen().clone() {
                Screen::Home => self.ui_home(ui),
                Screen::Exercise => self.ui_exercise(ui),
                Screen::Feedback {
                    result,
                    heard,
                    expected,
                } => self.ui_feedback(ui, result, heard, expected),
                Screen::Dictaphone => self.ui_dictaphone(ui),
                Screen::Settings => self.ui_settings(ui),
                Screen::Result { correct, total } => self.ui_result(ui, correct, total),
            }
        });
    }
}

impl UiApp {
    fn ui_home(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                RichText::new("SoftEcho")
                    .font(FontId::proportional(42.0))
                    .strong()
                    .color(Color32::from_rgb(20, 40, 60)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Восстановление речи · занятия дома")
                    .font(FontId::proportional(22.0))
                    .color(Color32::from_rgb(60, 80, 100)),
            );
            ui.add_space(24.0);
            ui.label(
                RichText::new(format!("Набор: {}", self.engine.pack().title))
                    .font(FontId::proportional(20.0)),
            );
            ui.label(
                RichText::new(format!(
                    "Пройдено занятий: {} · верных ответов: {}/{}",
                    self.engine.progress().sessions_completed,
                    self.engine.progress().total_correct,
                    self.engine.progress().total_answered
                ))
                .font(FontId::proportional(18.0))
                .color(Color32::DARK_GRAY),
            );

            ui.add_space(12.0);
            match self.engine.asr_status() {
                AsrStatus::Ready => ui.label(
                    RichText::new("Голос: готов (Vosk)")
                        .font(FontId::proportional(16.0))
                        .color(Color32::from_rgb(30, 120, 60)),
                ),
                AsrStatus::ModelMissing => ui.label(
                    RichText::new("Голос: модель не найдена — доступна самопроверка")
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                ),
                AsrStatus::Disabled => ui.label(
                    RichText::new("Голос: выключен в сборке (текстовый режим)")
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                ),
                AsrStatus::Error(e) => ui.label(
                    RichText::new(format!("Голос: {e}"))
                        .font(FontId::proportional(16.0))
                        .color(Color32::from_rgb(160, 60, 40)),
                ),
            };

            if let Some(err) = self.engine.load_error() {
                ui.colored_label(Color32::RED, err);
            }
            if let Some(err) = self.engine.save_error() {
                ui.colored_label(Color32::from_rgb(160, 60, 40), format!("Прогресс: {err}"));
            }

            ui.add_space(36.0);
            if big_button(ui, "Начать занятие", Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::StartSession);
            }
            ui.add_space(12.0);
            if big_button(ui, "Настройки", Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenSettings);
            }
            ui.add_space(12.0);
            let dictaphone_ok = matches!(self.engine.asr_status(), AsrStatus::Ready);
            if dictaphone_ok {
                if big_button(ui, "Диктофон", Color32::from_rgb(140, 60, 100)).clicked() {
                    self.engine.handle(Command::OpenDictaphone);
                }
            } else {
                ui.label(
                    RichText::new("Диктофон доступен при сборке с голосом (Vosk)")
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                );
            }
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            ui.label(
                RichText::new("Настройки")
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(16.0);

            match self.engine.asr_status() {
                AsrStatus::Ready => ui.label(
                    RichText::new("Голос: готов (Vosk)")
                        .font(FontId::proportional(20.0))
                        .color(Color32::from_rgb(30, 120, 60)),
                ),
                AsrStatus::ModelMissing => ui.label(
                    RichText::new("Голос: модель не найдена")
                        .font(FontId::proportional(20.0))
                        .color(Color32::from_rgb(150, 90, 30)),
                ),
                AsrStatus::Disabled => ui.label(
                    RichText::new("Голос: выключен в этой сборке")
                        .font(FontId::proportional(20.0))
                        .color(Color32::DARK_GRAY),
                ),
                AsrStatus::Error(e) => ui.label(
                    RichText::new(format!("Голос: {e}"))
                        .font(FontId::proportional(20.0))
                        .color(Color32::from_rgb(160, 60, 40)),
                ),
            };

            if let Some(dir) = self.engine.user_data_dir_display() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("Данные: {dir}"))
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                );
            }

            ui.add_space(20.0);
            match self.engine.model_download() {
                ModelDownloadState::Idle | ModelDownloadState::Succeeded => {
                    let can_download = !matches!(
                        self.engine.asr_status(),
                        AsrStatus::Disabled | AsrStatus::Ready
                    );
                    if can_download {
                        ui.label(
                            RichText::new(
                                "Скачать русскую модель Vosk (~45 МБ). Нужен интернет один раз.",
                            )
                            .font(FontId::proportional(18.0))
                            .color(Color32::DARK_GRAY),
                        );
                        ui.add_space(12.0);
                        if big_button(
                            ui,
                            "Скачать модель",
                            Color32::from_rgb(40, 110, 180),
                        )
                        .clicked()
                        {
                            self.engine.handle(Command::StartModelDownload);
                        }
                    } else if matches!(self.engine.asr_status(), AsrStatus::Ready) {
                        ui.label(
                            RichText::new("Модель уже на месте — перезапуск не нужен.")
                                .font(FontId::proportional(18.0))
                                .color(Color32::DARK_GRAY),
                        );
                    }
                }
                ModelDownloadState::Working { label, percent } => {
                    ui.label(
                        RichText::new(label)
                            .font(FontId::proportional(20.0))
                            .strong(),
                    );
                    if let Some(p) = percent {
                        ui.add_space(8.0);
                        ui.add(
                            egui::ProgressBar::new(f32::from(*p) / 100.0)
                                .text(format!("{p}%"))
                                .desired_width(320.0),
                        );
                    } else {
                        ui.add_space(8.0);
                        ui.spinner();
                    }
                }
                ModelDownloadState::Failed(err) => {
                    ui.colored_label(Color32::from_rgb(160, 60, 40), err);
                    ui.add_space(12.0);
                    if big_button(ui, "Повторить", Color32::from_rgb(40, 110, 180)).clicked() {
                        self.engine.handle(Command::StartModelDownload);
                    }
                }
            }

            if let Some(note) = self.engine.model_download_note() {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(note)
                        .font(FontId::proportional(18.0))
                        .color(Color32::from_rgb(30, 120, 60)),
                );
            }

            ui.add_space(28.0);
            if big_button(ui, "На главную", Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::LeaveSettings);
            }
        });
    }

    fn ui_exercise(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.engine.session() else {
            return;
        };
        let total = session.exercises.len();
        let idx = session.index;
        let progress_label = format!("Упражнение {} из {}", idx + 1, total);

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(progress_label)
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(16.0);
        });

        let exercise = match self.engine.current_exercise().cloned() {
            Some(e) => e,
            None => return,
        };

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(exercise.prompt())
                    .font(FontId::proportional(28.0))
                    .strong(),
            );
            ui.add_space(24.0);
        });

        match exercise {
            Exercise::ChooseWord { .. } => {
                let options = self
                    .engine
                    .session()
                    .map(|s| s.choice_options.clone())
                    .unwrap_or_default();
                ui.vertical_centered(|ui| {
                    for opt in &options {
                        if big_button(ui, opt, Color32::from_rgb(50, 90, 130)).clicked() {
                            self.engine.handle(Command::Submit(UserAnswer::Choice(opt.clone())));
                            return;
                        }
                        ui.add_space(10.0);
                    }
                });
            }
            Exercise::BuildPhrase { .. } => {
                self.ui_build_phrase(ui);
            }
            Exercise::ReadAloud { text, .. } => {
                self.ui_read_aloud(ui, &text);
            }
        }
    }

    fn ui_build_phrase(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.engine.session() else {
            return;
        };
        let picked_label = if session.picked.is_empty() {
            "Нажимайте слова по порядку".to_string()
        } else {
            session.picked.join(" ")
        };
        let pool = session.pool.clone();
        let can_check = !session.picked.is_empty() && session.pool.is_empty();

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(picked_label)
                    .font(FontId::proportional(26.0))
                    .color(Color32::from_rgb(20, 60, 40)),
            );
            ui.add_space(20.0);
        });

        ui.horizontal_wrapped(|ui| {
            ui.add_space(ui.available_width() * 0.1);
            let mut clicked: Option<usize> = None;
            for (i, w) in pool.iter().enumerate() {
                if big_button(ui, w, Color32::from_rgb(70, 100, 140)).clicked() {
                    clicked = Some(i);
                }
            }
            if let Some(i) = clicked {
                self.engine.handle(Command::PickPoolWord(i));
            }
        });

        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                if big_button(ui, "Сбросить", Color32::from_rgb(120, 90, 70)).clicked() {
                    self.engine.handle(Command::ResetBuildPhrase);
                }
                ui.add_space(12.0);
                if can_check && big_button(ui, "Проверить", Color32::from_rgb(40, 130, 90)).clicked()
                {
                    let parts = self
                        .engine
                        .session()
                        .map(|s| s.picked.clone())
                        .unwrap_or_default();
                    self.engine.handle(Command::Submit(UserAnswer::Phrase(parts)));
                }
            });
        });
    }

    fn ui_read_aloud(&mut self, ui: &mut egui::Ui, text: &str) {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(40.0))
                    .strong()
                    .color(Color32::from_rgb(15, 35, 55)),
            );
            ui.add_space(28.0);

            let asr_ready = matches!(self.engine.asr_status(), AsrStatus::Ready);
            let listening = self
                .engine
                .session()
                .map(|s| s.listening)
                .unwrap_or(false);

            if asr_ready {
                if listening {
                    if self.engine.please_wait() {
                        ui.label(
                            RichText::new(
                                "Подождите: распознаю накопленный звук. Говорить пока не нужно.",
                            )
                            .font(FontId::proportional(22.0))
                            .strong()
                            .color(Color32::from_rgb(150, 90, 30)),
                        );
                    } else {
                        ui.label(
                            RichText::new("Говорите… остановлюсь после паузы")
                                .font(FontId::proportional(22.0))
                                .color(Color32::from_rgb(140, 60, 100)),
                        );
                    }
                    let live = self
                        .engine
                        .session()
                        .map(|s| s.live_text.as_str())
                        .unwrap_or("");
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(if live.is_empty() {
                            "…"
                        } else {
                            live
                        })
                        .font(FontId::proportional(32.0))
                        .strong()
                        .color(Color32::from_rgb(20, 40, 60)),
                    );
                } else if big_button(ui, "Сказать", Color32::from_rgb(140, 60, 100)).clicked() {
                    self.engine.handle(Command::ListenExercise);
                }
                ui.add_space(12.0);
            }

            if let Some(err) = self.engine.session().and_then(|s| s.listen_error.clone()) {
                ui.colored_label(Color32::from_rgb(160, 60, 40), err);
                ui.add_space(8.0);
            }

            ui.label(
                RichText::new("Или отметьте сами:")
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(8.0);
            if listening {
                ui.label(
                    RichText::new("Подождите окончания записи…")
                        .font(FontId::proportional(18.0))
                        .color(Color32::DARK_GRAY),
                );
            } else {
                ui.horizontal(|ui| {
                    if big_button(ui, "Получилось", Color32::from_rgb(40, 130, 90)).clicked() {
                        self.engine.handle(Command::Submit(UserAnswer::ReadDone {
                            matched: true,
                            heard: None,
                        }));
                    }
                    ui.add_space(12.0);
                    if big_button(ui, "Не получилось", Color32::from_rgb(150, 70, 60)).clicked() {
                        self.engine.handle(Command::Submit(UserAnswer::ReadDone {
                            matched: false,
                            heard: None,
                        }));
                    }
                });
            }
        });
    }

    fn ui_feedback(
        &mut self,
        ui: &mut egui::Ui,
        result: CheckResult,
        heard: Option<String>,
        expected: Option<String>,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            let (text, color) = match result {
                CheckResult::Correct => ("Верно", Color32::from_rgb(30, 130, 70)),
                CheckResult::Incorrect => ("Неверно", Color32::from_rgb(160, 50, 40)),
            };
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(48.0))
                    .strong()
                    .color(color),
            );

            if result == CheckResult::Incorrect {
                if let Some(h) = &heard {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new("Услышала:")
                            .font(FontId::proportional(22.0))
                            .color(Color32::DARK_GRAY),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(h)
                            .font(FontId::proportional(36.0))
                            .strong()
                            .color(Color32::from_rgb(20, 40, 60)),
                    );
                } else {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("Модель ничего не распознала (или была самопроверка)")
                            .font(FontId::proportional(18.0))
                            .color(Color32::DARK_GRAY),
                    );
                }
                if let Some(exp) = &expected {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new(format!("Нужно было: {exp}"))
                            .font(FontId::proportional(22.0))
                            .color(Color32::from_rgb(80, 90, 100)),
                    );
                }
            } else if let Some(h) = &heard {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!("Услышала: {h}"))
                        .font(FontId::proportional(22.0))
                        .color(Color32::DARK_GRAY),
                );
            }

            ui.add_space(40.0);
            if big_button(ui, "Дальше", Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::AdvanceAfterFeedback);
            }
        });
    }

    fn ui_dictaphone(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            ui.label(
                RichText::new("Долгий диктофон")
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Говорите сколько нужно — текст копится и пишется в .txt на диск. Стоп — когда закончите.",
                )
                .font(FontId::proportional(18.0))
                .color(Color32::DARK_GRAY),
            );
            ui.add_space(20.0);

            if self.engine.dictaphone().listening {
                ui.label(
                    RichText::new("Идёт запись… (до 3 часов или Стоп)")
                        .font(FontId::proportional(22.0))
                        .color(Color32::from_rgb(140, 60, 100)),
                );
                if self.engine.please_wait() {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(
                            "Подождите: распознаю накопленный звук. Говорить пока не нужно.",
                        )
                        .font(FontId::proportional(22.0))
                        .strong()
                        .color(Color32::from_rgb(150, 90, 30)),
                    );
                }
                ui.add_space(12.0);
                if big_button(ui, "Стоп", Color32::from_rgb(150, 70, 60)).clicked() {
                    self.engine.handle(Command::StopDictaphone);
                }
            } else if big_button(ui, "Запись", Color32::from_rgb(140, 60, 100)).clicked() {
                self.engine.handle(Command::ListenDictaphone);
            }

            if let Some(err) = &self.engine.dictaphone().error {
                ui.add_space(12.0);
                ui.colored_label(Color32::from_rgb(160, 60, 40), err);
            }
            if let Some(note) = &self.engine.dictaphone().save_note {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(note)
                        .font(FontId::proportional(16.0))
                        .color(Color32::from_rgb(50, 90, 70)),
                );
            }

            // Хвост по байтам (без chars().count / полного clone каждый кадр).
            const UI_BYTES: usize = 12_000;
            let tr = self.engine.dictaphone().transcript.as_str();
            let live = self.engine.dictaphone().live_text.as_str();
            let truncated = tr.len() > UI_BYTES;
            let tr_tail = str_byte_tail(tr, UI_BYTES);

            if !tr.is_empty() || !live.is_empty() || self.engine.dictaphone().listening {
                ui.add_space(24.0);
                ui.label(
                    RichText::new(if truncated {
                        "Текст (хвост; полный — в .txt):"
                    } else {
                        "Текст:"
                    })
                    .font(FontId::proportional(20.0))
                    .color(Color32::DARK_GRAY),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().min(720.0));
                        if truncated {
                            ui.label(
                                RichText::new("…")
                                    .font(FontId::proportional(24.0))
                                    .color(Color32::DARK_GRAY),
                            );
                        }
                        if !tr_tail.is_empty() {
                            ui.label(
                                RichText::new(tr_tail)
                                    .font(FontId::proportional(24.0))
                                    .strong()
                                    .color(Color32::from_rgb(20, 40, 60)),
                            );
                        }
                        if !live.is_empty() {
                            ui.label(
                                RichText::new(live)
                                    .font(FontId::proportional(24.0))
                                    .color(Color32::from_rgb(80, 50, 100)),
                            );
                        } else if tr_tail.is_empty() {
                            ui.label(
                                RichText::new("…")
                                    .font(FontId::proportional(24.0))
                                    .strong()
                                    .color(Color32::from_rgb(20, 40, 60)),
                            );
                        }
                    });
            }

            ui.add_space(28.0);
            if !self.engine.dictaphone().listening {
                ui.horizontal(|ui| {
                    let has_text = !self.engine.dictaphone().transcript.is_empty();
                    if has_text
                        && big_button(ui, "Сохранить txt", Color32::from_rgb(40, 110, 90)).clicked()
                    {
                        self.engine.handle(Command::SaveDictaphone);
                    }
                    ui.add_space(12.0);
                    let can_clear = has_text
                        || !self.engine.dictaphone().live_text.is_empty()
                        || self.engine.dictaphone().error.is_some()
                        || self.engine.dictaphone().save_note.is_some();
                    if can_clear
                        && big_button(ui, "Очистить", Color32::from_rgb(120, 90, 70)).clicked()
                    {
                        self.engine.handle(Command::ClearDictaphone);
                    }
                    ui.add_space(12.0);
                    if big_button(ui, "Назад", Color32::from_rgb(90, 100, 110)).clicked() {
                        self.engine.handle(Command::LeaveDictaphone);
                    }
                });
            }
        });
    }

    fn ui_result(&mut self, ui: &mut egui::Ui, correct: u32, total: u32) {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.label(
                RichText::new("Занятие закончено")
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!("Верно: {correct} из {total}"))
                    .font(FontId::proportional(28.0)),
            );
            if let Some(err) = self.engine.save_error() {
                ui.add_space(12.0);
                ui.colored_label(
                    Color32::from_rgb(160, 60, 40),
                    format!("Не удалось сохранить прогресс: {err}"),
                );
            }
            ui.add_space(40.0);
            if big_button(ui, "На главный экран", Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::GoHome);
            }
            ui.add_space(12.0);
            if big_button(ui, "Ещё раз", Color32::from_rgb(40, 130, 90)).clicked() {
                self.engine.handle(Command::AgainSession);
            }
        });
    }
}

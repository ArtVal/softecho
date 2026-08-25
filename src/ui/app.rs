//! Клиентский UI (egui). Общается с движком только через Command / геттеры / tick.

use crate::engine::{
    AsrStatus, CheckResult, Command, Engine, Exercise, ExerciseStage, ModelDownloadState, Screen,
    SpeechRating, UserAnswer,
};
use crate::engine::warmup::{WARMUP_LINKS, WARMUP_SCHEMAS};
use crate::ui::theme::apply_theme;
use crate::ui::widgets::{back_to_menu_button, big_button, footer_buttons, screen_scroll, str_byte_tail};

use eframe::egui::{self, Color32, FontId, OpenUrl, RichText};

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

        if !matches!(self.engine.screen(), Screen::Home) {
            egui::TopBottomPanel::top("menu_back_bar")
                .resizable(false)
                .show_separator_line(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        if back_to_menu_button(ui).clicked() {
                            self.engine.handle(Command::GoHome);
                        }
                    });
                    ui.add_space(4.0);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            match self.engine.screen().clone() {
                Screen::Home => screen_scroll(ui, "home", |ui| self.ui_home(ui)),
                Screen::PackPick => screen_scroll(ui, "pack", |ui| self.ui_pack_pick(ui)),
                Screen::LevelPick => screen_scroll(ui, "level", |ui| self.ui_level_pick(ui)),
                Screen::Exercise => screen_scroll(ui, "ex", |ui| self.ui_exercise(ui)),
                Screen::Feedback {
                    result,
                    heard,
                    expected,
                } => screen_scroll(ui, "fb", |ui| self.ui_feedback(ui, result, heard, expected)),
                Screen::DiagnosisResult { level } => {
                    screen_scroll(ui, "diag_ok", |ui| self.ui_diagnosis_result(ui, level))
                }
                Screen::SpeechMap => {
                    screen_scroll(ui, "speech_map", |ui| self.ui_speech_map(ui))
                }
                Screen::Warmup => screen_scroll(ui, "warmup", |ui| self.ui_warmup(ui)),
                Screen::Dictaphone => self.ui_dictaphone(ui),
                Screen::Settings => screen_scroll(ui, "settings", |ui| self.ui_settings(ui)),
                Screen::Result { correct, total, unique } => {
                    screen_scroll(ui, "result", |ui| self.ui_result(ui, correct, total, unique))
                }
            }
        });
    }
}

impl UiApp {
    fn ui_home(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(16.0);
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
            ui.add_space(20.0);
            ui.label(
                RichText::new(format!("Набор: {}", self.engine.pack().title))
                    .font(FontId::proportional(20.0)),
            );
            ui.add_space(6.0);
            let level_text = match self.engine.level() {
                Some(l) => format!("Уровень: {}", l.label_ru()),
                None => "Уровень: не выбран".into(),
            };
            ui.label(
                RichText::new(level_text)
                    .font(FontId::proportional(18.0))
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "Занятий: {} · верно {}/{}",
                    self.engine.progress().sessions_completed,
                    self.engine.progress().total_correct,
                    self.engine.progress().total_answered
                ))
                .font(FontId::proportional(16.0))
                .color(Color32::DARK_GRAY),
            );

            if let Some(err) = self.engine.load_error() {
                ui.add_space(12.0);
                ui.colored_label(Color32::RED, err);
            }
            if let Some(err) = self.engine.save_error() {
                ui.colored_label(Color32::from_rgb(160, 60, 40), format!("Прогресс: {err}"));
            }

            ui.add_space(32.0);
            if big_button(ui, "Начать занятие", Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::StartSession);
            }
            ui.add_space(12.0);
            if big_button(ui, "Экспресс-диагностика", Color32::from_rgb(40, 130, 90)).clicked() {
                self.engine.handle(Command::StartDiagnosis);
            }
            ui.add_space(12.0);
            if big_button(ui, "Разминка", Color32::from_rgb(70, 120, 100)).clicked() {
                self.engine.handle(Command::OpenWarmup);
            }
            ui.add_space(12.0);
            if matches!(self.engine.asr_status(), AsrStatus::Ready) {
                if big_button(ui, "Диктофон", Color32::from_rgb(140, 60, 100)).clicked() {
                    self.engine.handle(Command::OpenDictaphone);
                }
            } else {
                ui.label(
                    RichText::new("Диктофон — в сборке с голосом (Vosk)")
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                );
            }
            ui.add_space(24.0);
            if big_button(ui, "Настройки", Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenSettings);
            }
        });
    }

    fn ui_pack_pick(&mut self, ui: &mut egui::Ui) {
        let current = self.engine.pack_id().to_string();
        ui.vertical_centered(|ui| {
            ui.add_space(28.0);
            ui.label(
                RichText::new("Набор упражнений")
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Тема занятий. Уровень и прогресс сохраняются.")
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(24.0);
            for entry in self.engine.pack_catalog() {
                let selected = entry.id == current;
                let fill = if selected {
                    Color32::from_rgb(40, 130, 90)
                } else {
                    Color32::from_rgb(40, 110, 180)
                };
                if big_button(ui, &entry.title, fill).clicked() {
                    self.engine.handle(Command::SetPack(entry.id));
                }
                ui.add_space(12.0);
            }
        });
    }

    fn ui_level_pick(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(28.0);
            ui.label(
                RichText::new("Уровень")
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Можно пропустить диагностику и выбрать ступень вручную.\n\
                     Занятие начнётся с этой ступени и выше.",
                )
                .font(FontId::proportional(18.0))
                .color(Color32::DARK_GRAY),
            );
            ui.add_space(24.0);
            for level in ExerciseStage::ALL {
                if level == ExerciseStage::Twister && !self.engine.twister_unlocked() {
                    ui.label(
                        RichText::new(
                            "Скороговорки — после уровня «Фразы» или когда ≥70% фраз «получается».",
                        )
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                    );
                    ui.add_space(12.0);
                    continue;
                }
                if big_button(ui, level.label_ru(), Color32::from_rgb(40, 110, 180)).clicked() {
                    self.engine.handle(Command::SetLevel(level));
                }
                ui.add_space(12.0);
            }
            ui.add_space(12.0);
            if big_button(ui, "Экспресс-диагностика", Color32::from_rgb(40, 130, 90)).clicked()
            {
                self.engine.handle(Command::StartDiagnosis);
            }
        });
    }

    fn ui_speech_map(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                RichText::new("Карта произнесения")
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("Набор: {}", self.engine.pack().title))
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Получается · почти · нужна практика — по результатам занятий и диагностики.\n\
                     Слабые места идут первыми в следующем занятии.",
                )
                .font(FontId::proportional(16.0))
                .color(Color32::DARK_GRAY),
            );
            ui.add_space(20.0);

            let entries = self.engine.speech_map_entries();
            let weak_n = entries
                .iter()
                .filter(|e| e.rating == SpeechRating::Weak)
                .count();
            let almost_n = entries
                .iter()
                .filter(|e| e.rating == SpeechRating::Almost)
                .count();
            let good_n = entries
                .iter()
                .filter(|e| e.rating == SpeechRating::Good)
                .count();
            let unknown_n = entries
                .iter()
                .filter(|e| e.rating == SpeechRating::Unknown)
                .count();
            if !entries.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "Нужна практика: {weak_n} · почти: {almost_n} · получается: {good_n} · ещё нет: {unknown_n}"
                    ))
                    .font(FontId::proportional(16.0))
                    .color(Color32::from_rgb(60, 80, 100)),
                );
                ui.add_space(12.0);
            }
            if entries.is_empty() {
                ui.label(
                    RichText::new("В этом наборе пока нет заданий.")
                        .font(FontId::proportional(18.0))
                        .color(Color32::DARK_GRAY),
                );
            } else {
                let mut current_stage: Option<ExerciseStage> = None;
                for entry in &entries {
                    if current_stage != Some(entry.stage) {
                        current_stage = Some(entry.stage);
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(entry.stage.label_ru())
                                .font(FontId::proportional(22.0))
                                .strong()
                                .color(Color32::from_rgb(40, 70, 100)),
                        );
                        ui.add_space(8.0);
                    }
                    let color = match entry.rating {
                        SpeechRating::Good => Color32::from_rgb(40, 130, 70),
                        SpeechRating::Almost => Color32::from_rgb(180, 130, 30),
                        SpeechRating::Weak => Color32::from_rgb(180, 60, 50),
                        SpeechRating::Unknown => Color32::from_rgb(120, 120, 130),
                    };
                    let detail = if entry.attempts > 0 {
                        format!(
                            "{} — {}/{}",
                            entry.rating.label_ru(),
                            entry.correct,
                            entry.attempts
                        )
                    } else {
                        entry.rating.label_ru().to_string()
                    };
                    ui.label(
                        RichText::new(format!("{} · {}", entry.label, detail))
                            .font(FontId::proportional(18.0))
                            .color(color),
                    );
                    ui.add_space(4.0);
                }
            }
        });
    }

    fn ui_warmup(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                RichText::new("Разминка")
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Перед занятием: губы, язык, выдох. Это не проверка — только подготовка.",
                )
                .font(FontId::proportional(18.0))
                .color(Color32::DARK_GRAY),
            );
            ui.add_space(20.0);

            for schema in WARMUP_SCHEMAS {
                ui.label(
                    RichText::new(schema.title)
                        .font(FontId::proportional(26.0))
                        .strong()
                        .color(Color32::from_rgb(40, 70, 100)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(schema.diagram)
                        .font(FontId::monospace(18.0))
                        .color(Color32::from_rgb(20, 40, 60)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(schema.how)
                        .font(FontId::proportional(18.0))
                        .color(Color32::from_rgb(50, 70, 90)),
                );
                ui.add_space(20.0);
            }

            ui.label(
                RichText::new("Видео снаружи (откроется в браузере)")
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("Чужие ролики — смотреть можно, в приложение не вшиваем.")
                    .font(FontId::proportional(15.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(12.0);

            for link in WARMUP_LINKS {
                if big_button(ui, link.label, Color32::from_rgb(60, 100, 140)).clicked() {
                    ui.ctx().open_url(OpenUrl::new_tab(link.url));
                }
                ui.add_space(8.0);
            }

            ui.add_space(16.0);
            ui.label(
                RichText::new("После схем удобно набор «Артикуляция: па-та-ка».")
                    .font(FontId::proportional(16.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(12.0);
            if big_button(ui, "Открыть па-та-ка", Color32::from_rgb(40, 130, 90)).clicked() {
                self.engine.handle(Command::SetPack("odk".into()));
            }
            ui.add_space(8.0);
            if big_button(ui, "Начать занятие", Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::StartSession);
            }
            ui.add_space(8.0);
            if big_button(ui, "Назад", Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::LeaveWarmup);
            }
            ui.add_space(24.0);
        });
    }

    fn ui_diagnosis_result(&mut self, ui: &mut egui::Ui, level: ExerciseStage) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                RichText::new("Диагностика готова")
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!("Уровень: {}", level.label_ru()))
                    .font(FontId::proportional(32.0))
                    .strong()
                    .color(Color32::from_rgb(30, 100, 70)),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new("Сохранён. Занятие пойдёт с этой ступени.\nКарта произнесения тоже обновлена.")
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(36.0);
            if big_button(ui, "Начать занятие", Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::StartSession);
            }
            ui.add_space(12.0);
            if big_button(ui, "Карта произнесения", Color32::from_rgb(100, 80, 150)).clicked()
            {
                self.engine.handle(Command::OpenSpeechMap);
            }
            ui.add_space(12.0);
            if big_button(ui, "Выбрать другой", Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenLevelPick);
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

            ui.label(
                RichText::new("Набор и уровень")
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("Набор: {}", self.engine.pack().title))
                    .font(FontId::proportional(18.0)),
            );
            let level_text = match self.engine.level() {
                Some(l) => format!("Уровень: {}", l.label_ru()),
                None => "Уровень: не выбран".into(),
            };
            ui.label(
                RichText::new(level_text)
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(12.0);
            if big_button(ui, "Сменить набор", Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenPackPick);
            }
            ui.add_space(8.0);
            if big_button(ui, "Выбрать уровень", Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenLevelPick);
            }
            ui.add_space(8.0);
            if big_button(
                ui,
                "Карта произнесения",
                Color32::from_rgb(100, 80, 150),
            )
            .clicked()
            {
                self.engine.handle(Command::OpenSpeechMap);
            }
            ui.add_space(8.0);
            if big_button(ui, "Разминка", Color32::from_rgb(70, 120, 100)).clicked() {
                self.engine.handle(Command::OpenWarmup);
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Слабые места возвращаются в занятии; «Не повторять» — пропуск до конца урока.",
                )
                .font(FontId::proportional(15.0))
                .color(Color32::DARK_GRAY),
            );

            ui.add_space(24.0);
            ui.label(
                RichText::new("Голос")
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(8.0);

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

            if let Some(err) = self.engine.save_error() {
                ui.add_space(12.0);
                ui.colored_label(
                    Color32::from_rgb(160, 60, 40),
                    format!("Прогресс: {err}"),
                );
            }
        });
    }

    fn ui_exercise(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.engine.session() else {
            return;
        };
        let total = session.exercises.len();
        let idx = session.index;
        let stage_label = session
            .exercises
            .get(idx)
            .map(|e| e.stage().label_ru())
            .unwrap_or("");
        let mode = if self.engine.session_is_diagnosis() {
            "Диагностика"
        } else {
            "Занятие"
        };
        let progress_label = if stage_label.is_empty() {
            format!("{mode} · {} из {}", idx + 1, total)
        } else {
            format!("{mode} · {stage_label} · {} из {}", idx + 1, total)
        };

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(progress_label)
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            if self.engine.current_exercise_is_practice_repeat() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Повтор — нужна практика")
                        .font(FontId::proportional(16.0))
                        .color(Color32::from_rgb(180, 100, 40)),
                );
            }
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
                let is_twister = matches!(
                    self.engine.current_exercise().map(|e| e.stage()),
                    Some(ExerciseStage::Twister)
                );
                self.ui_read_aloud(ui, &text, is_twister);
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

    fn ui_read_aloud(&mut self, ui: &mut egui::Ui, text: &str, is_twister: bool) {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(40.0))
                    .strong()
                    .color(Color32::from_rgb(15, 35, 55)),
            );
            if is_twister {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(
                        "Сначала медленно по словам → трудные места отдельно → целиком медленно → чуть быстрее.\n\
                         «Готово» или самопроверка — принять попытку.",
                    )
                    .font(FontId::proportional(17.0))
                    .color(Color32::from_rgb(80, 90, 100)),
                );
            }
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
                    ui.add_space(16.0);
                    if big_button(ui, "Готово", Color32::from_rgb(40, 130, 90)).clicked() {
                        self.engine.handle(Command::StopExerciseListen);
                    }
                } else if big_button(ui, "Сказать", Color32::from_rgb(140, 60, 100)).clicked() {
                    self.engine.handle(Command::ListenExercise);
                }
                ui.add_space(12.0);
            }

            if let Some(err) = self.engine.session().and_then(|s| s.listen_error.clone()) {
                ui.colored_label(Color32::from_rgb(160, 60, 40), err);
                ui.add_space(8.0);
            }

            if !listening {
                let heard = self
                    .engine
                    .session()
                    .map(|s| s.live_text.as_str())
                    .unwrap_or("")
                    .to_string();
                let hint = self.engine.session().and_then(|s| s.asr_hint_ok);
                if !heard.is_empty() {
                    ui.label(
                        RichText::new(format!("Услышала: {heard}"))
                            .font(FontId::proportional(22.0))
                            .color(Color32::from_rgb(20, 40, 60)),
                    );
                    if let Some(ok) = hint {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(if ok {
                                "Похоже верно — отметьте сами"
                            } else {
                                "Похоже иначе — отметьте сами"
                            })
                            .font(FontId::proportional(18.0))
                            .color(Color32::DARK_GRAY),
                        );
                    }
                    ui.add_space(12.0);
                }

                if self.engine.has_last_clip() {
                    let playing = self.engine.is_playing_clip();
                    let label = if playing {
                        "Стоп прослушивания"
                    } else {
                        "Послушать"
                    };
                    let color = if playing {
                        Color32::from_rgb(120, 90, 70)
                    } else {
                        Color32::from_rgb(60, 100, 140)
                    };
                    if big_button(ui, label, color).clicked() {
                        if playing {
                            self.engine.handle(Command::StopPlayback);
                        } else {
                            self.engine.handle(Command::PlayLastClip);
                        }
                    }
                    ui.add_space(12.0);
                }
            }

            ui.label(
                RichText::new("Или отметьте сами:")
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(8.0);
            if listening {
                ui.label(
                    RichText::new("Или нажмите «Готово», если слог уже сказали")
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

            if self.engine.has_last_clip() {
                ui.add_space(20.0);
                let playing = self.engine.is_playing_clip();
                let label = if playing {
                    "Стоп прослушивания"
                } else {
                    "Послушать запись"
                };
                let color = if playing {
                    Color32::from_rgb(120, 90, 70)
                } else {
                    Color32::from_rgb(60, 100, 140)
                };
                if big_button(ui, label, color).clicked() {
                    if playing {
                        self.engine.handle(Command::StopPlayback);
                    } else {
                        self.engine.handle(Command::PlayLastClip);
                    }
                }
            }

            ui.add_space(40.0);
            if result == CheckResult::Incorrect && self.engine.session_is_practice() {
                if let Some(left) = self.engine.feedback_requeues_left() {
                    let hint = if left == 0 {
                        "В этом занятии больше не вернём — лимит или «не повторять».".into()
                    } else {
                        format!(
                            "Вернём позже — чем чаще ошибка, тем раньше. Ещё до {left} раз в этом занятии."
                        )
                    };
                    ui.label(
                        RichText::new(hint)
                            .font(FontId::proportional(16.0))
                            .color(Color32::DARK_GRAY),
                    );
                }
                ui.add_space(16.0);
                if big_button(ui, "Дальше", Color32::from_rgb(40, 110, 180)).clicked() {
                    self.engine.handle(Command::AdvanceAfterFeedback);
                }
                ui.add_space(12.0);
                if big_button(ui, "Не повторять", Color32::from_rgb(90, 100, 120)).clicked() {
                    self.engine.handle(Command::SkipRepeatAndAdvance);
                }
            } else if big_button(ui, "Дальше", Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::AdvanceAfterFeedback);
            }
        });
    }

    fn ui_dictaphone(&mut self, ui: &mut egui::Ui) {
        // Нижняя полоса всегда на виду (Windows: не прячется за панель задач).
        egui::TopBottomPanel::bottom("dictaphone_footer")
            .resizable(false)
            .show_separator_line(true)
            .show_inside(ui, |ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if self.engine.dictaphone().listening {
                        if big_button(ui, "Стоп", Color32::from_rgb(150, 70, 60)).clicked() {
                            self.engine.handle(Command::StopDictaphone);
                        }
                    } else {
                        footer_buttons(ui, |ui| {
                            let has_text = !self.engine.dictaphone().transcript.is_empty();
                            if has_text
                                && big_button(ui, "Сохранить txt", Color32::from_rgb(40, 110, 90))
                                    .clicked()
                            {
                                self.engine.handle(Command::SaveDictaphone);
                            }
                            if ui.available_width() >= 640.0 {
                                ui.add_space(12.0);
                            } else {
                                ui.add_space(8.0);
                            }
                            let can_clear = has_text
                                || !self.engine.dictaphone().live_text.is_empty()
                                || self.engine.dictaphone().error.is_some()
                                || self.engine.dictaphone().save_note.is_some();
                            if can_clear
                                && big_button(ui, "Очистить", Color32::from_rgb(120, 90, 70))
                                    .clicked()
                            {
                                self.engine.handle(Command::ClearDictaphone);
                            }
                            if ui.available_width() >= 640.0 {
                                ui.add_space(12.0);
                            } else {
                                ui.add_space(8.0);
                            }
                            if big_button(ui, "Назад", Color32::from_rgb(90, 100, 110)).clicked() {
                                self.engine.handle(Command::LeaveDictaphone);
                            }
                        });
                    }
                });
                ui.add_space(12.0);
            });

        egui::ScrollArea::vertical()
            .id_salt("dictaphone_body")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let compact = ui.available_width() < 520.0 || ui.available_height() < 520.0;
                let title_size = if compact { 28.0 } else { 36.0 };
                let body_size = if compact { 16.0 } else { 18.0 };
                let text_size = if compact { 20.0 } else { 24.0 };
                ui.vertical_centered(|ui| {
                    ui.add_space(if compact { 8.0 } else { 16.0 });
                    ui.label(
                        RichText::new("Долгий диктофон")
                            .font(FontId::proportional(title_size))
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Говорите сколько нужно — текст копится в .txt. Стоп — когда закончите.",
                        )
                        .font(FontId::proportional(body_size))
                        .color(Color32::DARK_GRAY),
                    );
                    ui.add_space(if compact { 12.0 } else { 20.0 });

                    if self.engine.dictaphone().listening {
                        ui.label(
                            RichText::new("Идёт запись… (Стоп внизу)")
                                .font(FontId::proportional(body_size + 2.0))
                                .color(Color32::from_rgb(140, 60, 100)),
                        );
                        if self.engine.please_wait() {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(
                                    "Подождите: распознаю звук. Говорить пока не нужно.",
                                )
                                .font(FontId::proportional(body_size + 2.0))
                                .strong()
                                .color(Color32::from_rgb(150, 90, 30)),
                            );
                        }
                    } else if big_button(ui, "Запись", Color32::from_rgb(140, 60, 100)).clicked() {
                        self.engine.handle(Command::ListenDictaphone);
                    }

                    if !self.engine.dictaphone().listening && self.engine.has_last_clip() {
                        ui.add_space(12.0);
                        let playing = self.engine.is_playing_clip();
                        let label = if playing {
                            "Стоп прослушивания"
                        } else {
                            "Послушать"
                        };
                        let color = if playing {
                            Color32::from_rgb(120, 90, 70)
                        } else {
                            Color32::from_rgb(60, 100, 140)
                        };
                        if big_button(ui, label, color).clicked() {
                            if playing {
                                self.engine.handle(Command::StopPlayback);
                            } else {
                                self.engine.handle(Command::PlayLastClip);
                            }
                        }
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
                        let text_h = if compact {
                            (ui.available_height() * 0.35).clamp(72.0, 160.0)
                        } else {
                            (ui.available_height() * 0.45).clamp(96.0, 280.0)
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("dictaphone_text")
                            .max_height(text_h)
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                if truncated {
                                    ui.label(
                                        RichText::new("…")
                                            .font(FontId::proportional(text_size))
                                            .color(Color32::DARK_GRAY),
                                    );
                                }
                                if !tr_tail.is_empty() {
                                    ui.label(
                                        RichText::new(tr_tail)
                                            .font(FontId::proportional(text_size))
                                            .strong()
                                            .color(Color32::from_rgb(20, 40, 60)),
                                    );
                                }
                                if !live.is_empty() {
                                    ui.label(
                                        RichText::new(live)
                                            .font(FontId::proportional(text_size))
                                            .color(Color32::from_rgb(80, 50, 100)),
                                    );
                                } else if tr_tail.is_empty() {
                                    ui.label(
                                        RichText::new("…")
                                            .font(FontId::proportional(text_size))
                                            .strong()
                                            .color(Color32::from_rgb(20, 40, 60)),
                                    );
                                }
                            });
                    }
                    ui.add_space(16.0);
                });
            });
    }

    fn ui_result(&mut self, ui: &mut egui::Ui, correct: u32, total: u32, unique: u32) {
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
            if total > unique {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "Заданий в плане: {unique} · с повторами слабых: {total}"
                    ))
                    .font(FontId::proportional(16.0))
                    .color(Color32::DARK_GRAY),
                );
            }
            if let Some(err) = self.engine.save_error() {
                ui.add_space(12.0);
                ui.colored_label(
                    Color32::from_rgb(160, 60, 40),
                    format!("Не удалось сохранить прогресс: {err}"),
                );
            }
            ui.add_space(40.0);
            if big_button(ui, "Ещё раз", Color32::from_rgb(40, 130, 90)).clicked() {
                self.engine.handle(Command::AgainSession);
            }
            ui.add_space(12.0);
            if big_button(ui, "Карта произнесения", Color32::from_rgb(100, 80, 150)).clicked()
            {
                self.engine.handle(Command::OpenSpeechMap);
            }
        });
    }
}

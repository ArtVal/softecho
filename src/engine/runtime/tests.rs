use super::*;
use crate::engine::data::with_temp_xdg_data_home;
use crate::engine::protocol::{ModelDownloadState, Screen};

#[test]
fn set_pack_manual() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::SetPack("daily".into()));
    assert_eq!(eng.pack().title, "Дом и быт");
    assert_eq!(eng.pack_id(), "daily");
    assert!(matches!(eng.screen(), Screen::Home));
}

#[test]
fn start_session_without_level_opens_picker() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = None;
    eng.handle(Command::StartSession);
    assert!(matches!(eng.screen(), Screen::LevelPick));
    assert!(eng.session().is_none());
}

#[test]
fn start_session_opens_exercise() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Syllable);
    assert!(matches!(eng.screen(), Screen::Home));
    eng.handle(Command::StartSession);
    assert!(matches!(eng.screen(), Screen::Exercise));
    assert!(eng.session().is_some());
    let s = eng.session().unwrap();
    assert!(!s.exercises.is_empty());
    assert_eq!(s.index, 0);
    let stages: Vec<_> = s.exercises.iter().map(Exercise::stage).collect();
    assert_eq!(stages.first(), Some(&crate::engine::ExerciseStage::Syllable));
    let mut seen_word = false;
    let mut seen_phrase = false;
    let mut seen_twister = false;
    for st in &stages {
        match st {
            crate::engine::ExerciseStage::Sound => {
                panic!("звук отфильтрован уровнем «слоги»");
            }
            crate::engine::ExerciseStage::Syllable => {
                assert!(!seen_word && !seen_phrase && !seen_twister);
            }
            crate::engine::ExerciseStage::Word => {
                seen_word = true;
                assert!(!seen_phrase && !seen_twister);
            }
            crate::engine::ExerciseStage::Phrase => {
                seen_phrase = true;
                assert!(!seen_twister);
            }
            crate::engine::ExerciseStage::Twister => seen_twister = true,
        }
    }
    assert!(seen_word && seen_phrase);
}

#[test]
fn set_level_manual_and_filter_practice() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::SetLevel(ExerciseStage::Word));
    assert_eq!(eng.level(), Some(ExerciseStage::Word));
    assert!(matches!(eng.screen(), Screen::Home));
    eng.handle(Command::StartSession);
    let s = eng.session().unwrap();
    assert!(s.exercises.iter().all(|e| e.stage() >= ExerciseStage::Word));
    assert!(!s.exercises.iter().any(|e| e.stage() == ExerciseStage::Syllable));
}

#[test]
fn diagnosis_sets_level_automatically() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::StartDiagnosis);
    assert!(eng.session_is_diagnosis());
    assert!(matches!(eng.screen(), Screen::Exercise));
    // Все ответы верные → уровень «Фразы».
    loop {
        let Some(ex) = eng.current_exercise().cloned() else {
            break;
        };
        match ex {
            Exercise::ChooseWord { answer, .. } => {
                eng.handle(Command::Submit(UserAnswer::Choice(answer)));
            }
            Exercise::BuildPhrase { answer, .. } => {
                let parts: Vec<_> = answer.split_whitespace().map(str::to_string).collect();
                eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
            }
            Exercise::ReadAloud { .. } => {
                eng.handle(Command::Submit(UserAnswer::ReadDone {
                    matched: true,
                    heard: None,
                }));
            }
        }
        eng.handle(Command::AdvanceAfterFeedback);
        if matches!(eng.screen(), Screen::DiagnosisResult { .. }) {
            break;
        }
    }
    assert!(matches!(
        eng.screen(),
        Screen::DiagnosisResult {
            level: ExerciseStage::Phrase
        }
    ));
    assert_eq!(eng.level(), Some(ExerciseStage::Phrase));
    // Диагностика не считает обычное занятие.
    assert_eq!(eng.progress().sessions_completed, 0);
}

#[test]
fn diagnosis_weak_syllables_sets_syllable_level() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::StartDiagnosis);
    loop {
        let Some(ex) = eng.current_exercise().cloned() else {
            break;
        };
        let ok = ex.stage() != ExerciseStage::Syllable;
        match ex {
            Exercise::ChooseWord { answer, .. } => {
                if ok {
                    eng.handle(Command::Submit(UserAnswer::Choice(answer)));
                } else {
                    eng.handle(Command::Submit(UserAnswer::Choice("__нет__".into())));
                }
            }
            Exercise::BuildPhrase { answer, .. } => {
                let parts: Vec<_> = if ok {
                    answer.split_whitespace().map(str::to_string).collect()
                } else {
                    vec!["нет".into()]
                };
                eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
            }
            Exercise::ReadAloud { .. } => {
                eng.handle(Command::Submit(UserAnswer::ReadDone {
                    matched: ok,
                    heard: None,
                }));
            }
        }
        eng.handle(Command::AdvanceAfterFeedback);
        if matches!(eng.screen(), Screen::DiagnosisResult { .. }) {
            break;
        }
    }
    assert_eq!(eng.level(), Some(ExerciseStage::Syllable));
}

#[test]
fn diagnosis_weak_sounds_sets_sound_level() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::StartDiagnosis);
    loop {
        let Some(ex) = eng.current_exercise().cloned() else {
            break;
        };
        let ok = ex.stage() != ExerciseStage::Sound;
        match ex {
            Exercise::ChooseWord { answer, .. } => {
                if ok {
                    eng.handle(Command::Submit(UserAnswer::Choice(answer)));
                } else {
                    eng.handle(Command::Submit(UserAnswer::Choice("__нет__".into())));
                }
            }
            Exercise::BuildPhrase { answer, .. } => {
                let parts: Vec<_> = if ok {
                    answer.split_whitespace().map(str::to_string).collect()
                } else {
                    vec!["нет".into()]
                };
                eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
            }
            Exercise::ReadAloud { .. } => {
                eng.handle(Command::Submit(UserAnswer::ReadDone {
                    matched: ok,
                    heard: None,
                }));
            }
        }
        eng.handle(Command::AdvanceAfterFeedback);
        if matches!(eng.screen(), Screen::DiagnosisResult { .. }) {
            break;
        }
    }
    assert_eq!(eng.level(), Some(ExerciseStage::Sound));
}

#[test]
fn choose_word_flow_to_feedback() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Syllable);
    eng.handle(Command::StartSession);
    // Дойти до ChooseWord, если первый другой — листаем через неверный ответ нельзя без feedback.
    // Берём упражнение из сессии и сабмитим подходящий тип.
    let ex = eng.current_exercise().cloned().unwrap();
    match ex {
        Exercise::ChooseWord { answer, .. } => {
            eng.handle(Command::Submit(UserAnswer::Choice(answer)));
        }
        Exercise::BuildPhrase { answer, .. } => {
            let parts: Vec<String> = answer.split_whitespace().map(str::to_string).collect();
            eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
        }
        Exercise::ReadAloud { .. } => {
            eng.handle(Command::Submit(UserAnswer::ReadDone {
                matched: true,
                heard: None,
            }));
        }
    }
    assert!(matches!(eng.screen(), Screen::Feedback { .. }));
    eng.handle(Command::AdvanceAfterFeedback);
    // Либо следующее упражнение, либо результат (если одно).
    assert!(matches!(
        eng.screen(),
        Screen::Exercise | Screen::Result { .. }
    ));
}

#[test]
fn pick_pool_word_and_undo() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Syllable);
    eng.handle(Command::StartSession);
    // Найти BuildPhrase
    let mut found = false;
    for _ in 0..eng.session().unwrap().exercises.len() {
        if matches!(eng.current_exercise(), Some(Exercise::BuildPhrase { .. })) {
            found = true;
            break;
        }
        // форсируем переход: сдаём текущее
        let ex = eng.current_exercise().cloned().unwrap();
        match ex {
            Exercise::ChooseWord { answer, .. } => {
                eng.handle(Command::Submit(UserAnswer::Choice(answer)));
            }
            Exercise::BuildPhrase { answer, .. } => {
                let parts: Vec<_> = answer.split_whitespace().map(str::to_string).collect();
                eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
            }
            Exercise::ReadAloud { .. } => {
                eng.handle(Command::Submit(UserAnswer::ReadDone {
                    matched: true,
                    heard: None,
                }));
            }
        }
        eng.handle(Command::AdvanceAfterFeedback);
        if matches!(eng.screen(), Screen::Result { .. }) {
            break;
        }
    }
    if !found {
        return; // набор без BuildPhrase — пропускаем
    }
    let pool_len = eng.session().unwrap().pool.len();
    assert!(pool_len > 0);
    eng.handle(Command::PickPoolWord(0));
    assert_eq!(eng.session().unwrap().pool.len(), pool_len - 1);
    assert_eq!(eng.session().unwrap().picked.len(), 1);
    eng.handle(Command::UndoPickedWord);
    assert_eq!(eng.session().unwrap().picked.len(), 0);
    assert_eq!(eng.session().unwrap().pool.len(), pool_len);
}

#[test]
fn go_home_clears_session() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Syllable);
    eng.handle(Command::StartSession);
    eng.handle(Command::GoHome);
    assert!(matches!(eng.screen(), Screen::Home));
    assert!(eng.session().is_none());
    assert!(!eng.please_wait());
}

#[test]
fn open_speech_map_and_leave() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::OpenSpeechMap);
    assert!(matches!(eng.screen(), Screen::SpeechMap));
    assert!(!eng.speech_map_entries().is_empty());
    eng.handle(Command::LeaveSpeechMap);
    assert!(matches!(eng.screen(), Screen::Home));
}

#[test]
fn open_warmup_and_leave() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::OpenWarmup);
    assert!(matches!(eng.screen(), Screen::Warmup));
    eng.handle(Command::LeaveWarmup);
    assert!(matches!(eng.screen(), Screen::Home));
}

#[test]
fn open_progress_and_leave() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::OpenProgress);
    assert!(matches!(eng.screen(), Screen::ProgressReport));
    let text = eng.progress_report_text();
    assert!(text.contains("SoftEcho"));
    eng.handle(Command::LeaveProgress);
    assert!(matches!(eng.screen(), Screen::Home));
}

#[test]
fn export_progress_report_writes_file() {
    with_temp_xdg_data_home(|_tmp| {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::ExportProgressReport);
        let note = eng.report_export_note().expect("note").to_string();
        assert!(
            note.contains("Отчёт сохранён") || note.contains("Report saved"),
            "{note}"
        );
        assert!(
            note.contains("softecho-report_") && note.contains(".txt"),
            "{note}"
        );
        let path = note.split(": ").nth(1).expect("path after colon");
        let body = std::fs::read_to_string(path).expect("report file");
        assert!(body.contains("SoftEcho"));
    });
}

#[test]
fn open_pack_editor_builtin_prompts_clone() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::OpenPackEditor);
    assert!(matches!(eng.screen(), Screen::PackEditor));
    assert!(eng.pack_editor().is_none());
    eng.handle(Command::LeavePackEditor);
    assert!(matches!(eng.screen(), Screen::Home));
}

#[test]
fn pack_editor_dirty_blocks_leave_until_discard() {
    with_temp_xdg_data_home(|_tmp| {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::ClonePackForEdit);
        assert!(
            eng.pack_editor().is_some(),
            "clone failed: {:?}",
            eng.load_error()
        );
        eng.handle(Command::EditorDisable(0));
        assert!(eng.pack_editor().unwrap().dirty);
        eng.handle(Command::LeavePackEditor);
        assert!(matches!(eng.screen(), Screen::PackEditor));
        assert!(eng.pack_editor().unwrap().error.is_some());
        eng.handle(Command::DiscardPackEditor);
        assert!(matches!(eng.screen(), Screen::Home));
        assert!(eng.pack_editor().is_none());
    });
}

fn tiny_test_pack() -> ExercisePack {
    ExercisePack {
        title: "test".into(),
        exercises: vec![
            Exercise::ChooseWord {
                stage: Some(ExerciseStage::Word),
                prompt: "q".into(),
                options: vec!["дом".into(), "чай".into()],
                answer: "дом".into(),
                image: None,
            },
            Exercise::ChooseWord {
                stage: Some(ExerciseStage::Word),
                prompt: "q2".into(),
                options: vec!["чай".into(), "стол".into()],
                answer: "чай".into(),
                image: None,
            },
        ],
    }
}

fn wrong_answer_for(ex: &Exercise) -> UserAnswer {
    match ex {
        Exercise::ChooseWord { options, answer, .. } => {
            let wrong = options
                .iter()
                .find(|o| *o != answer)
                .cloned()
                .unwrap_or_else(|| "__нет__".into());
            UserAnswer::Choice(wrong)
        }
        Exercise::BuildPhrase { answer, .. } => {
            let mut parts: Vec<String> = answer.split_whitespace().map(str::to_string).collect();
            if parts.len() >= 2 {
                parts.swap(0, 1);
            }
            UserAnswer::Phrase(parts)
        }
        Exercise::ReadAloud { .. } => UserAnswer::ReadDone {
            matched: false,
            heard: None,
        },
    }
}

#[test]
fn incorrect_practice_requeues_on_advance() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Word);
    eng.pack = tiny_test_pack();
    eng.handle(Command::StartSession);
    let len = eng.session().unwrap().exercises.len();
    let ex = eng.current_exercise().cloned().unwrap();
    eng.handle(Command::Submit(wrong_answer_for(&ex)));
    eng.handle(Command::AdvanceAfterFeedback);
    assert_eq!(eng.session().unwrap().exercises.len(), len + 1);
    assert!(matches!(eng.screen(), Screen::Exercise));
}

#[test]
fn skip_repeat_does_not_requeue() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Word);
    eng.pack = tiny_test_pack();
    eng.handle(Command::StartSession);
    let len = eng.session().unwrap().exercises.len();
    let ex = eng.current_exercise().cloned().unwrap();
    eng.handle(Command::Submit(wrong_answer_for(&ex)));
    eng.handle(Command::SkipRepeatAndAdvance);
    assert_eq!(eng.session().unwrap().exercises.len(), len);
}

#[test]
fn requeue_capped_per_key() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Word);
    eng.pack = ExercisePack {
        title: "one".into(),
        exercises: vec![Exercise::ChooseWord {
            stage: Some(ExerciseStage::Word),
            prompt: "q".into(),
            options: vec!["дом".into(), "чай".into()],
            answer: "дом".into(),
            image: None,
        }],
    };
    eng.handle(Command::StartSession);
    let mut max_len = 1;
    for _ in 0..6 {
        if !matches!(eng.screen(), Screen::Exercise) {
            break;
        }
        let ex = eng.current_exercise().cloned().unwrap();
        eng.handle(Command::Submit(wrong_answer_for(&ex)));
        eng.handle(Command::AdvanceAfterFeedback);
        if let Some(s) = eng.session() {
            max_len = max_len.max(s.exercises.len());
        }
    }
    assert_eq!(max_len, 4);
}

#[test]
fn skip_repeat_blocks_later_requeue_same_key() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Word);
    eng.pack = ExercisePack {
        title: "same-key".into(),
        exercises: vec![
            Exercise::ChooseWord {
                stage: Some(ExerciseStage::Word),
                prompt: "q1".into(),
                options: vec!["дом".into(), "чай".into()],
                answer: "дом".into(),
                image: None,
            },
            Exercise::ChooseWord {
                stage: Some(ExerciseStage::Word),
                prompt: "q2".into(),
                options: vec!["дом".into(), "стол".into()],
                answer: "дом".into(),
                image: None,
            },
        ],
    };
    eng.handle(Command::StartSession);
    let len = eng.session().unwrap().exercises.len();
    let ex = eng.current_exercise().cloned().unwrap();
    eng.handle(Command::Submit(wrong_answer_for(&ex)));
    eng.handle(Command::SkipRepeatAndAdvance);
    assert_eq!(eng.session().unwrap().exercises.len(), len);
    assert!(matches!(eng.screen(), Screen::Exercise));
    let ex2 = eng.current_exercise().cloned().unwrap();
    eng.handle(Command::Submit(wrong_answer_for(&ex2)));
    assert_eq!(eng.feedback_requeues_left(), Some(0));
    let len_before = eng.session().unwrap().exercises.len();
    eng.handle(Command::AdvanceAfterFeedback);
    if let Some(s) = eng.session() {
        assert_eq!(s.exercises.len(), len_before);
    } else {
        assert_eq!(len_before, len);
        assert!(matches!(eng.screen(), Screen::Result { .. }));
    }
}

#[test]
fn higher_session_boost_comes_earlier() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Word);
    // «дом» слабее в карте — стартует первым (без случайного порядка).
    eng.progress.speech_map.record("дом", false);
    eng.progress.speech_map.record("дом", false);
    eng.pack = ExercisePack {
        title: "two".into(),
        exercises: vec![
            Exercise::ChooseWord {
                stage: Some(ExerciseStage::Word),
                prompt: "a".into(),
                options: vec!["дом".into(), "чай".into()],
                answer: "дом".into(),
                image: None,
            },
            Exercise::ChooseWord {
                stage: Some(ExerciseStage::Word),
                prompt: "b".into(),
                options: vec!["чай".into(), "стол".into()],
                answer: "чай".into(),
                image: None,
            },
        ],
    };
    eng.handle(Command::StartSession);
    let ex0 = eng.current_exercise().cloned().unwrap();
    match &ex0 {
        Exercise::ChooseWord { answer, .. } => assert_eq!(answer, "дом"),
        _ => panic!("ожидали «дом» первым"),
    }
    eng.handle(Command::Submit(wrong_answer_for(&ex0)));
    eng.handle(Command::AdvanceAfterFeedback);
    let ex1 = eng.current_exercise().cloned().unwrap();
    eng.handle(Command::Submit(wrong_answer_for(&ex1)));
    eng.handle(Command::AdvanceAfterFeedback);
    let ex2 = eng.current_exercise().cloned().unwrap();
    eng.handle(Command::Submit(wrong_answer_for(&ex2)));
    eng.handle(Command::AdvanceAfterFeedback);
    let next = eng.current_exercise().cloned().unwrap();
    match next {
        Exercise::ChooseWord { answer, .. } => assert_eq!(answer, "дом"),
        _ => panic!("ожидали повтор «дом»"),
    }
}

#[test]
fn diagnosis_does_not_requeue() {
    let mut eng = Engine::new_logic_only();
    eng.pack = tiny_test_pack();
    eng.handle(Command::StartDiagnosis);
    let len = eng.session().unwrap().exercises.len();
    let ex = eng.current_exercise().cloned().unwrap();
    eng.handle(Command::Submit(wrong_answer_for(&ex)));
    eng.handle(Command::AdvanceAfterFeedback);
    assert_eq!(eng.session().unwrap().exercises.len(), len);
}

#[test]
fn submit_updates_speech_map() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Syllable);
    eng.progress.speech_map = Default::default();
    eng.handle(Command::StartSession);
    let ex = eng.current_exercise().cloned().unwrap();
    let key = ex.map_key().expect("ключ");
    match ex {
        Exercise::ChooseWord { answer, .. } => {
            eng.handle(Command::Submit(UserAnswer::Choice(answer)));
        }
        Exercise::BuildPhrase { answer, .. } => {
            let parts: Vec<_> = answer.split_whitespace().map(str::to_string).collect();
            eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
        }
        Exercise::ReadAloud { .. } => {
            eng.handle(Command::Submit(UserAnswer::ReadDone {
                matched: true,
                heard: None,
            }));
        }
    }
    let stat = eng.progress().speech_map.items.get(&key).unwrap();
    assert_eq!(stat.attempts, 1);
    assert_eq!(stat.correct, 1);
}

#[test]
fn dictaphone_open_and_leave() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::OpenDictaphone);
    assert!(matches!(eng.screen(), Screen::Dictaphone));
    eng.handle(Command::LeaveDictaphone);
    assert!(matches!(eng.screen(), Screen::Home));
}

#[test]
fn go_home_from_dictaphone_clears_buffer() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::OpenDictaphone);
    eng.dictaphone.transcript = "черновик".into();
    eng.handle(Command::GoHome);
    assert!(matches!(eng.screen(), Screen::Home));
    assert!(eng.dictaphone.transcript.is_empty());
}

#[test]
fn settings_screen_from_home() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::OpenSettings);
    assert!(matches!(eng.screen(), Screen::Settings));
    eng.handle(Command::LeaveSettings);
    assert!(matches!(eng.screen(), Screen::Home));
    assert!(eng.model_download_note().is_none());
}

#[test]
fn set_language_switches_default_pack() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::SetPack("daily".into()));
    assert_eq!(eng.pack_id(), "daily");
    eng.handle(Command::OpenSettings);
    eng.handle(Command::SetLanguage(AppLanguage::En));
    assert_eq!(eng.language(), AppLanguage::En);
    assert_eq!(eng.pack_id(), "starter_en");
    assert!(eng.pack().title.contains("Sounds") || eng.pack().title.contains("syllables"));
    assert!(matches!(eng.screen(), Screen::Settings));
    let ids: Vec<_> = eng.pack_catalog().into_iter().map(|e| e.id).collect();
    assert!(ids.contains(&"starter_en".into()));
    assert!(!ids.contains(&"daily".into()));
    eng.handle(Command::SetLanguage(AppLanguage::Ru));
    assert_eq!(eng.language(), AppLanguage::Ru);
    assert_eq!(eng.pack_id(), "starter");
}

#[test]
fn set_language_drops_in_flight_download() {
    let mut eng = Engine::new_logic_only();
    let (tx, rx) = std::sync::mpsc::channel();
    eng.model_download_rx = Some(rx);
    eng.model_download = ModelDownloadState::Working {
        label: "old".into(),
        percent: Some(10),
    };
    eng.handle(Command::SetLanguage(AppLanguage::En));
    assert!(eng.model_download_rx.is_none());
    assert!(matches!(eng.model_download(), ModelDownloadState::Idle));
    // Старый отправитель больше не доставляет в движок.
    let _ = tx.send(DownloadMsg::Percent(99));
    let mut tick = TickResult::default();
    eng.poll_model_download(&mut tick);
    assert!(matches!(eng.model_download(), ModelDownloadState::Idle));
}

#[test]
fn simple_mode_starts_sound_without_level() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = None;
    eng.handle(Command::SetSimpleMode(true));
    assert!(eng.simple_mode());
    eng.handle(Command::StartSession);
    assert_eq!(eng.level(), Some(ExerciseStage::Sound));
    assert!(matches!(eng.screen(), Screen::Exercise));
}

#[test]
#[cfg(not(feature = "asr"))]
fn start_model_download_noop_without_asr() {
    let mut eng = Engine::new_logic_only();
    eng.handle(Command::StartModelDownload);
    assert!(matches!(eng.model_download(), ModelDownloadState::Idle));
}

#[test]
#[cfg(feature = "asr")]
fn start_model_download_begins_when_model_missing() {
    with_temp_xdg_data_home(|_tmp| {
        let mut eng = Engine::new_logic_only();
        assert!(matches!(eng.asr_status(), AsrStatus::ModelMissing));
        eng.handle(Command::StartModelDownload);
        assert!(matches!(
            eng.model_download(),
            ModelDownloadState::Working { .. }
        ));
        eng.cancel_model_download();
    });
}

#[test]
fn poll_download_done_updates_state_after_reload() {
    let (tx, rx) = mpsc::channel();
    tx.send(DownloadMsg::Done).unwrap();
    drop(tx);
    let mut eng = Engine::new_logic_only();
    eng.test_download_rx(rx);
    let tick = eng.tick();
    assert!(tick.want_repaint);
    match eng.asr_status() {
        AsrStatus::Ready => {
            assert!(matches!(
                eng.model_download(),
                ModelDownloadState::Succeeded
            ));
            assert_eq!(
                eng.model_download_note(),
                Some("Модель установлена. Голос готов.")
            );
        }
        _ => {
            assert!(matches!(
                eng.model_download(),
                ModelDownloadState::Failed(_)
            ));
            assert!(eng.model_download_note().is_none());
        }
    }
}

#[test]
fn poll_download_err_sets_failed() {
    let (tx, rx) = mpsc::channel();
    tx.send(DownloadMsg::Err("сеть".into())).unwrap();
    drop(tx);
    let mut eng = Engine::new_logic_only();
    eng.test_download_rx(rx);
    eng.tick();
    assert!(matches!(
        eng.model_download(),
        ModelDownloadState::Failed(e) if e == "сеть"
    ));
}

#[test]
fn poll_download_percent_updates_progress() {
    let (tx, rx) = mpsc::channel();
    tx.send(DownloadMsg::Phase("Скачиваю…".into())).unwrap();
    tx.send(DownloadMsg::Percent(42)).unwrap();
    drop(tx);
    let mut eng = Engine::new_logic_only();
    eng.test_download_rx(rx);
    eng.tick();
    assert!(matches!(
        eng.model_download(),
        ModelDownloadState::Working {
            label,
            percent: Some(42)
        } if label == "Скачиваю…"
    ));
}

#[test]
fn tick_repaints_while_download_working() {
    let mut eng = Engine::new_logic_only();
    eng.model_download = ModelDownloadState::Working {
        label: "Скачиваю…".into(),
        percent: Some(10),
    };
    let tick = eng.tick();
    assert!(tick.want_repaint);
    assert_eq!(tick.repaint_after, Some(Duration::from_millis(200)));
}

#[test]
fn tick_idle_is_quiet() {
    let mut eng = Engine::new_logic_only();
    let t = eng.tick();
    assert!(!t.want_repaint);
    assert!(t.repaint_after.is_none());
}

fn submit_current_correct(eng: &mut Engine) {
    let Some(ex) = eng.current_exercise().cloned() else {
        return;
    };
    match ex {
        Exercise::ChooseWord { answer, .. } => {
            eng.handle(Command::Submit(UserAnswer::Choice(answer)));
        }
        Exercise::BuildPhrase { answer, .. } => {
            let parts: Vec<_> = answer.split_whitespace().map(str::to_string).collect();
            eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
        }
        Exercise::ReadAloud { .. } => {
            eng.handle(Command::Submit(UserAnswer::ReadDone {
                matched: true,
                heard: None,
            }));
        }
    }
}

/// Smoke: экраны UI открываются/закрываются через Command (контракт app.rs).
#[test]
fn ui_navigation_smoke_open_leave_and_gohome() {
    let mut eng = Engine::new_logic_only();

    let opens = [
        (Command::OpenPackPick, "PackPick"),
        (Command::OpenLevelPick, "LevelPick"),
        (Command::OpenSpeechMap, "SpeechMap"),
        (Command::OpenProgress, "ProgressReport"),
        (Command::OpenWarmup, "Warmup"),
        (Command::OpenSettings, "Settings"),
        (Command::OpenDictaphone, "Dictaphone"),
        (Command::OpenPackEditor, "PackEditor"),
    ];

    for (open, name) in opens {
        eng.handle(Command::GoHome);
        assert!(matches!(eng.screen(), Screen::Home));
        eng.handle(open.clone());
        let ok = match name {
            "PackPick" => matches!(eng.screen(), Screen::PackPick),
            "LevelPick" => matches!(eng.screen(), Screen::LevelPick),
            "SpeechMap" => matches!(eng.screen(), Screen::SpeechMap),
            "ProgressReport" => matches!(eng.screen(), Screen::ProgressReport),
            "Warmup" => matches!(eng.screen(), Screen::Warmup),
            "Settings" => matches!(eng.screen(), Screen::Settings),
            "Dictaphone" => matches!(eng.screen(), Screen::Dictaphone),
            "PackEditor" => matches!(eng.screen(), Screen::PackEditor),
            _ => false,
        };
        assert!(ok, "после {open:?} экран {name}, получили {:?}", eng.screen());
        eng.handle(Command::GoHome);
        assert!(matches!(eng.screen(), Screen::Home), "GoHome с {open:?}");
    }

    eng.handle(Command::OpenPackPick);
    eng.handle(Command::LeavePackPick);
    assert!(matches!(eng.screen(), Screen::Home));

    eng.handle(Command::OpenLevelPick);
    eng.handle(Command::LeaveLevelPick);
    assert!(matches!(eng.screen(), Screen::Home));

    eng.handle(Command::SetSimpleMode(true));
    assert!(eng.simple_mode());
    eng.handle(Command::SetSimpleMode(false));
    assert!(!eng.simple_mode());
}

/// Занятие до Result → AgainSession снова открывает Exercise.
#[test]
fn ui_practice_reaches_result_then_again() {
    let mut eng = Engine::new_logic_only();
    eng.pack = tiny_test_pack();
    eng.progress.level = Some(ExerciseStage::Word);
    eng.handle(Command::StartSession);
    assert!(matches!(eng.screen(), Screen::Exercise));

    let mut guard = 0;
    while !matches!(eng.screen(), Screen::Result { .. }) {
        guard += 1;
        assert!(guard < 40, "зациклились без Result");
        if matches!(eng.screen(), Screen::Exercise) {
            submit_current_correct(&mut eng);
        }
        if matches!(eng.screen(), Screen::Feedback { .. }) {
            eng.handle(Command::AdvanceAfterFeedback);
        }
    }

    match eng.screen() {
        Screen::Result {
            correct,
            total,
            unique,
        } => {
            assert!(*total >= 1);
            assert_eq!(*correct, *total);
            assert!(*unique >= 1);
        }
        other => panic!("ожидали Result, получили {other:?}"),
    }

    eng.handle(Command::AgainSession);
    assert!(matches!(eng.screen(), Screen::Exercise));
    assert!(eng.session().is_some());
}

#[test]
fn listen_gate_busy_join_sets_error() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Word);
    eng.pack = tiny_test_pack();
    eng.handle(Command::StartSession);
    assert!(eng.session().is_some());

    let release = Arc::new(AtomicBool::new(false));
    eng.test_inject_busy_listen_join(Arc::clone(&release));
    assert!(eng.test_listen_worker_busy());
    assert!(eng.listen_rx.is_none());

    eng.handle(Command::ListenExercise);
    let err = eng
        .session()
        .and_then(|s| s.listen_error.as_deref())
        .expect("listen_error после gate");
    assert!(
        err.contains("останавливается"),
        "ожидали gate-текст, получили: {err}"
    );

    release.store(true, Ordering::SeqCst);
    while eng.test_listen_worker_busy() {
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn dictaphone_done_err_saves_transcript() {
    with_temp_xdg_data_home(|tmp| {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::OpenDictaphone);
        let path = tmp.join("dictaphone").join("done_err.txt");
        eng.test_set_dictaphone_save(path.clone(), "уже было".into());

        let (tx, rx) = mpsc::channel();
        tx.send(ListenEvent::Done(Err("микрофон".into()))).unwrap();
        drop(tx);
        eng.test_inject_listen(rx, ListenPurpose::Dictaphone, None);

        eng.tick();
        assert_eq!(eng.dictaphone().error.as_deref(), Some("микрофон"));
        assert!(
            eng.dictaphone()
                .save_note
                .as_deref()
                .is_some_and(|n| n.contains("Сохранено")),
            "save_note: {:?}",
            eng.dictaphone().save_note
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("файл после Done(Err)"),
            "уже было"
        );
    });
}

#[test]
fn abort_drain_exercise_done_err_sets_listen_error() {
    let mut eng = Engine::new_logic_only();
    eng.progress.level = Some(ExerciseStage::Word);
    eng.pack = tiny_test_pack();
    eng.handle(Command::StartSession);

    let (tx, rx) = mpsc::channel();
    tx.send(ListenEvent::Done(Err("сбой записи".into())))
        .unwrap();
    drop(tx);
    eng.test_inject_listen(rx, ListenPurpose::Exercise, Some("дом".into()));

    eng.test_abort_listen();
    assert_eq!(
        eng.session().and_then(|s| s.listen_error.clone()),
        Some("сбой записи".into())
    );
}

#[test]
fn play_last_clip_while_busy_sets_pending_replay() {
    let mut eng = Engine::new_logic_only();
    eng.test_set_last_clip(vec![1, 2, 3, 4]);
    eng.test_set_playback_busy(true);
    assert!(eng.test_playback_busy());
    assert!(!eng.test_playback_pending_replay());

    eng.handle(Command::PlayLastClip);

    assert!(
        eng.test_playback_pending_replay(),
        "при busy PlayLastClip должен поставить pending_replay"
    );
}

#[test]
fn pending_recognizer_reload_waits_for_join() {
    let mut eng = Engine::new_logic_only();
    eng.test_set_pending_recognizer_reload(false);
    let release = Arc::new(AtomicBool::new(false));
    eng.test_inject_busy_listen_join(Arc::clone(&release));
    assert!(eng.test_listen_worker_busy());

    eng.test_reload_recognizer();
    assert!(
        eng.test_pending_recognizer_reload(),
        "при busy join reload должен отложить"
    );
    assert!(eng.test_listen_worker_busy());

    release.store(true, Ordering::SeqCst);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while eng.test_listen_worker_busy() {
        assert!(
            std::time::Instant::now() < deadline,
            "join не завершился вовремя"
        );
        thread::sleep(Duration::from_millis(5));
    }

    eng.tick();
    assert!(
        !eng.test_pending_recognizer_reload(),
        "tick должен снять pending после join"
    );
    assert!(!eng.test_listen_worker_busy());
}

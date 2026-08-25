//! Разминка до занятия: схемы движений + внешние ссылки на видео.
//! Не тип упражнения и не встроенный плеер.

/// Одна схема (крупный текст / псевдорисунок).
#[derive(Debug, Clone, Copy)]
pub struct WarmupSchema {
    #[allow(dead_code)]
    pub title: &'static str,
    pub diagram: &'static str,
    #[allow(dead_code)]
    pub how: &'static str,
}

/// Ссылка «смотреть снаружи» (чужой ролик — не вшивать).
#[derive(Debug, Clone, Copy)]
pub struct WarmupLink {
    #[allow(dead_code)]
    pub label: &'static str,
    pub url: &'static str,
}

pub const WARMUP_SCHEMAS: &[WarmupSchema] = &[
    WarmupSchema {
        title: "Губы",
        diagram: "  (  · ·  )\n   \\___/\n\n  ( ·   · )\n   \\___/\n    |||",
        how: "Сначала широко «улыбка», потом губы в «трубочку». По 3–5 раз, без спешки.",
    },
    WarmupSchema {
        title: "Язык",
        diagram: "    /\\\n   /  \\\n  |    |\n   \\__/\n\n  ←  ■  →",
        how: "Кончик языка вверх к нёбу, потом влево и вправо. Рот приоткрыт, без напряжения шеи.",
    },
    WarmupSchema {
        title: "Выдох",
        diagram: "  (лёгкие)\n     ↓\n   ≈≈≈≈≈\n   с-с-с…",
        how: "Спокойный вдох носом, долгий выдох ртом со звуком «с-с-с» или «ф-ф-ф». 3 раза.",
    },
];

pub const WARMUP_LINKS: &[WarmupLink] = &[
    WarmupLink {
        label: "Викторова — сайт (артикуляционная гимнастика)",
        url: "https://logo-vav.ru/",
    },
    WarmupLink {
        label: "Сергеева — урок при афазии Брока (Rutube)",
        url: "https://rutube.ru/video/398b1ff9de17389903123ed99ed26522/",
    },
    WarmupLink {
        label: "ГКБ №52 — упражнения при афазии (Rutube)",
        url: "https://rutube.ru/video/f3fa74003f7f663e4390c37ac8e51fd7/",
    },
    WarmupLink {
        label: "Начальные фонетические упражнения (~36 мин)",
        url: "https://rutube.ru/video/d31cfa3d1a927a0649e32e3a32fafa3e/",
    },
];

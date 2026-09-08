use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Chinese,
    TraditionalChinese,
    Japanese,
    Korean,
    German,
    French,
    Spanish,
}

pub const LANGUAGES: &[(&str, &str)] = &[
    ("system", "System"),
    ("en", "English"),
    ("zh-CN", "简体中文"),
    ("zh-TW", "繁體中文"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("de", "Deutsch"),
    ("fr", "Français"),
    ("es", "Español"),
];

impl Language {
    pub fn resolve(preference: &str) -> Self {
        let locale = if preference == "system" || preference.is_empty() {
            system_locale()
        } else {
            preference
        };
        Self::from_locale(locale)
    }
    pub fn from_locale(locale: &str) -> Self {
        let locale = locale.to_ascii_lowercase().replace('_', "-");
        if locale.starts_with("zh") {
            if ["tw", "hk", "mo", "hant"]
                .iter()
                .any(|part| locale.split('-').any(|value| value == *part))
            {
                Self::TraditionalChinese
            } else {
                Self::Chinese
            }
        } else if locale.starts_with("ja") {
            Self::Japanese
        } else if locale.starts_with("ko") {
            Self::Korean
        } else if locale.starts_with("de") {
            Self::German
        } else if locale.starts_with("fr") {
            Self::French
        } else if locale.starts_with("es") {
            Self::Spanish
        } else {
            Self::English
        }
    }
    pub fn t(self, key: &str) -> &str {
        STRINGS
            .iter()
            .find(|(source, _)| *source == key)
            .map(|(_, row)| row[self as usize])
            .unwrap_or(key)
    }
}

pub fn system_locale() -> &'static str {
    static LOCALE: OnceLock<String> = OnceLock::new();
    LOCALE.get_or_init(detect_system_locale)
}

#[cfg(windows)]
fn detect_system_locale() -> String {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
        fn LCIDToLocaleName(locale: u32, name: *mut u16, count: i32, flags: u32) -> i32;
    }
    let mut name = [0u16; 85];
    // SAFETY: The fixed buffer meets LOCALE_NAME_MAX_LENGTH, and the API writes
    // at most the supplied number of UTF-16 code units.
    let length = unsafe {
        LCIDToLocaleName(
            u32::from(GetUserDefaultUILanguage()),
            name.as_mut_ptr(),
            name.len() as i32,
            0,
        )
    };
    if length > 1 {
        String::from_utf16_lossy(&name[..length as usize - 1])
    } else {
        "en-US".into()
    }
}
#[cfg(target_os = "macos")]
fn detect_system_locale() -> String {
    // Finder launches do not reliably set LANG; AppleLanguages reflects the
    // user's ordered system UI languages, independent of terminal environment.
    std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| {
            value
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && *line != "(" && *line != ")")
                .map(|line| line.trim_matches(['"', ',', ' ']).to_owned())
        })
        .unwrap_or_else(|| "en-US".into())
}
#[cfg(not(any(windows, target_os = "macos")))]
fn detect_system_locale() -> String {
    std::env::var("LANG").unwrap_or_else(|_| "en-US".into())
}

const STRINGS: &[(&str, [&str; 8])] = &[
    (
        "Workspace",
        [
            "Workspace",
            "工作区",
            "工作區",
            "ワークスペース",
            "작업 공간",
            "Arbeitsbereich",
            "Espace de travail",
            "Espacio de trabajo",
        ],
    ),
    (
        "Sessions",
        [
            "Sessions",
            "会话",
            "工作階段",
            "セッション",
            "세션",
            "Sitzungen",
            "Sessions",
            "Sesiones",
        ],
    ),
    (
        "Shells",
        [
            "Shells",
            "命令解释器",
            "命令殼層",
            "シェル",
            "셸",
            "Shells",
            "Interpréteurs",
            "Intérpretes",
        ],
    ),
    (
        "Settings",
        [
            "Settings",
            "设置",
            "設定",
            "設定",
            "설정",
            "Einstellungen",
            "Paramètres",
            "Configuración",
        ],
    ),
    (
        "Sidebar",
        [
            "Sidebar",
            "侧边栏",
            "側邊欄",
            "サイドバー",
            "사이드바",
            "Seitenleiste",
            "Barre latérale",
            "Barra lateral",
        ],
    ),
    (
        "New terminal",
        [
            "New terminal",
            "新建终端",
            "新增終端機",
            "新しいターミナル",
            "새 터미널",
            "Neues Terminal",
            "Nouveau terminal",
            "Nueva terminal",
        ],
    ),
    (
        "One workspace. Every shell.",
        [
            "One workspace. Every shell.",
            "一个工作区，容纳所有 Shell。",
            "一個工作區，容納所有 Shell。",
            "すべてのシェルを一か所に。",
            "모든 셸을 한곳에서.",
            "Alle Shells an einem Ort.",
            "Tous vos shells au même endroit.",
            "Todos tus shells en un lugar.",
        ],
    ),
    (
        "Make it yours",
        [
            "Make it yours",
            "个性化设置",
            "個人化設定",
            "自分好みにカスタマイズ",
            "나만의 설정",
            "Dein Terminal",
            "Personnaliser",
            "Personaliza tu terminal",
        ],
    ),
    (
        "Settings help",
        [
            "Appearance updates immediately. Shell and environment changes apply to new tabs.",
            "外观立即生效；Shell 与环境变量变更对新标签生效。",
            "外觀立即生效；Shell 與環境變數變更套用至新分頁。",
            "外観はすぐに反映されます。シェルと環境の変更は新しいタブに適用されます。",
            "모양은 즉시 적용됩니다. 셸과 환경 변경은 새 탭에 적용됩니다.",
            "Darstellung wirkt sofort. Shell- und Umgebungsänderungen gelten für neue Tabs.",
            "L’apparence change immédiatement. Le shell et l’environnement changent pour les nouveaux onglets.",
            "La apariencia cambia al instante. El shell y el entorno se aplican a las nuevas pestañas.",
        ],
    ),
    (
        "Terminal font",
        [
            "Terminal font",
            "终端字号",
            "終端機字級",
            "文字サイズ",
            "터미널 글꼴 크기",
            "Schriftgröße",
            "Taille du texte",
            "Tamaño de texto",
        ],
    ),
    (
        "Command suggestions",
        [
            "Command suggestions",
            "命令自动提示",
            "命令自動提示",
            "コマンド候補",
            "명령 제안",
            "Befehlsvorschläge",
            "Suggestions de commandes",
            "Sugerencias de comandos",
        ],
    ),
    (
        "On",
        [
            "On", "开启", "開啟", "オン", "켜짐", "Ein", "Activé", "Activado",
        ],
    ),
    (
        "Off",
        [
            "Off",
            "关闭",
            "關閉",
            "オフ",
            "꺼짐",
            "Aus",
            "Désactivé",
            "Desactivado",
        ],
    ),
    (
        "Environment",
        [
            "Environment",
            "环境变量",
            "環境變數",
            "環境変数",
            "환경 변수",
            "Umgebung",
            "Environnement",
            "Entorno",
        ],
    ),
    (
        "Inherited",
        [
            "Inherited",
            "继承系统",
            "繼承系統",
            "システムから継承",
            "시스템에서 상속",
            "Geerbt",
            "Hérité",
            "Heredado",
        ],
    ),
    (
        "Global overrides",
        [
            "global overrides",
            "项全局覆盖",
            "項全域覆寫",
            "個の上書き",
            "전역 재정의",
            "globale Vorgaben",
            "remplacements globaux",
            "valores globales",
        ],
    ),
    (
        "Language",
        [
            "Language",
            "界面语言",
            "介面語言",
            "表示言語",
            "표시 언어",
            "Sprache",
            "Langue",
            "Idioma",
        ],
    ),
    (
        "System",
        [
            "System",
            "跟随系统",
            "跟隨系統",
            "システム設定",
            "시스템 설정",
            "System",
            "Système",
            "Sistema",
        ],
    ),
    (
        "Theme",
        [
            "Theme",
            "主题皮肤",
            "主題外觀",
            "テーマ",
            "테마",
            "Design",
            "Thème",
            "Tema",
        ],
    ),
    (
        "Midnight",
        [
            "Midnight",
            "午夜薄荷",
            "午夜薄荷",
            "ミッドナイト",
            "미드나이트",
            "Mitternacht",
            "Minuit",
            "Medianoche",
        ],
    ),
    (
        "Light",
        [
            "Light",
            "明亮",
            "明亮",
            "ライト",
            "밝게",
            "Hell",
            "Clair",
            "Claro",
        ],
    ),
    (
        "Custom",
        [
            "Custom",
            "自定义",
            "自訂",
            "カスタム",
            "사용자 지정",
            "Benutzerdefiniert",
            "Personnalisé",
            "Personalizado",
        ],
    ),
    (
        "Edit config.toml",
        [
            "Edit config.toml",
            "编辑配置文件",
            "編輯設定檔",
            "設定ファイルを編集",
            "설정 파일 편집",
            "Konfiguration bearbeiten",
            "Modifier la configuration",
            "Editar configuración",
        ],
    ),
    (
        "Reload configuration",
        [
            "Reload configuration",
            "重新加载配置",
            "重新載入設定",
            "設定を再読み込み",
            "설정 다시 불러오기",
            "Konfiguration neu laden",
            "Recharger la configuration",
            "Recargar configuración",
        ],
    ),
    (
        "Settings shortcuts",
        [
            "Ctrl + Shift + R reloads · Esc closes settings",
            "Ctrl + Shift + R 重新加载 · Esc 关闭设置",
            "Ctrl + Shift + R 重新載入 · Esc 關閉設定",
            "Ctrl + Shift + R で再読み込み · Esc で閉じる",
            "Ctrl + Shift + R 다시 불러오기 · Esc 닫기",
            "Ctrl + Shift + R neu laden · Esc schließen",
            "Ctrl + Shift + R recharger · Esc fermer",
            "Ctrl + Shift + R recargar · Esc cerrar",
        ],
    ),
    (
        "Ready",
        [
            "Ready",
            "就绪",
            "就緒",
            "準備完了",
            "준비됨",
            "Bereit",
            "Prêt",
            "Listo",
        ],
    ),
    (
        "Running",
        [
            "Running",
            "运行中",
            "執行中",
            "実行中",
            "실행 중",
            "Aktiv",
            "En cours",
            "En ejecución",
        ],
    ),
    (
        "Exited",
        [
            "Exited",
            "已退出",
            "已結束",
            "終了",
            "종료됨",
            "Beendet",
            "Terminé",
            "Finalizado",
        ],
    ),
    (
        "Bundled",
        [
            "bundled",
            "内置",
            "內建",
            "同梱",
            "내장",
            "integriert",
            "intégré",
            "incluido",
        ],
    ),
    (
        "Dismiss",
        [
            "Dismiss",
            "关闭提示",
            "關閉提示",
            "閉じる",
            "닫기",
            "Schließen",
            "Fermer",
            "Cerrar",
        ],
    ),
    (
        "Open a new terminal",
        [
            "Open a new terminal",
            "打开新终端",
            "開啟新終端機",
            "ターミナルを開く",
            "새 터미널 열기",
            "Terminal öffnen",
            "Ouvrir un terminal",
            "Abrir una terminal",
        ],
    ),
    (
        "Launcher shortcuts",
        [
            "↑ ↓ to choose · Enter to open · Esc to close",
            "↑ ↓ 选择 · Enter 打开 · Esc 关闭",
            "↑ ↓ 選擇 · Enter 開啟 · Esc 關閉",
            "↑ ↓ 選択 · Enter 開く · Esc 閉じる",
            "↑ ↓ 선택 · Enter 열기 · Esc 닫기",
            "↑ ↓ auswählen · Enter öffnen · Esc schließen",
            "↑ ↓ choisir · Entrée ouvrir · Esc fermer",
            "↑ ↓ elegir · Intro abrir · Esc cerrar",
        ],
    ),
    (
        "Your next command starts here.",
        [
            "Your next command starts here.",
            "从这里开始下一条命令。",
            "從這裡開始下一個命令。",
            "次のコマンドはここから。",
            "다음 명령을 여기서 시작하세요.",
            "Dein nächster Befehl beginnt hier.",
            "Votre prochaine commande commence ici.",
            "Tu próximo comando empieza aquí.",
        ],
    ),
    (
        "Empty help",
        [
            "Open Bash, PowerShell, or your own shell in a new tab.",
            "在新标签中打开 Bash、PowerShell 或自定义 Shell。",
            "在新分頁開啟 Bash、PowerShell 或自訂 Shell。",
            "新しいタブで Bash、PowerShell、その他のシェルを開きます。",
            "새 탭에서 Bash, PowerShell 또는 사용자 셸을 여세요.",
            "Bash, PowerShell oder eine eigene Shell im neuen Tab öffnen.",
            "Ouvrez Bash, PowerShell ou votre shell dans un nouvel onglet.",
            "Abre Bash, PowerShell u otro shell en una nueva pestaña.",
        ],
    ),
    (
        "Find",
        [
            "Find",
            "查找",
            "尋找",
            "検索",
            "찾기",
            "Suchen",
            "Rechercher",
            "Buscar",
        ],
    ),
    (
        "matches",
        [
            "matches",
            "处匹配",
            "處符合",
            "件一致",
            "개 일치",
            "Treffer",
            "résultats",
            "coincidencias",
        ],
    ),
    (
        "Find shortcuts",
        [
            "Enter next · Esc close",
            "Enter 下一处 · Esc 关闭",
            "Enter 下一處 · Esc 關閉",
            "Enter 次へ · Esc 閉じる",
            "Enter 다음 · Esc 닫기",
            "Enter weiter · Esc schließen",
            "Entrée suivant · Esc fermer",
            "Intro siguiente · Esc cerrar",
        ],
    ),
    (
        "Suggestions",
        [
            "Suggestions",
            "命令提示",
            "命令提示",
            "コマンド候補",
            "명령 제안",
            "Vorschläge",
            "Suggestions",
            "Sugerencias",
        ],
    ),
    (
        "Suggestion shortcuts",
        [
            "Alt + → accepts · Tab completes in the shell",
            "Alt + → 采纳提示 · Tab 使用 Shell 补全",
            "Alt + → 採用提示 · Tab 使用 Shell 補全",
            "Alt + → 採用 · Tab シェル補完",
            "Alt + → 적용 · Tab 셸 자동 완성",
            "Alt + → übernehmen · Tab Shell-Vervollständigung",
            "Alt + → accepter · Tab compléter",
            "Alt + → aceptar · Tab completar",
        ],
    ),
    (
        "Session ended",
        [
            "Session exited. Ctrl + Shift + T opens a new terminal.",
            "会话已退出。按 Ctrl + Shift + T 新建终端。",
            "工作階段已結束。按 Ctrl + Shift + T 新增終端機。",
            "セッション終了。Ctrl + Shift + T で新しいターミナルを開きます。",
            "세션이 종료되었습니다. Ctrl + Shift + T로 새 터미널을 여세요.",
            "Sitzung beendet. Ctrl + Shift + T öffnet ein neues Terminal.",
            "Session terminée. Ctrl + Shift + T ouvre un terminal.",
            "Sesión finalizada. Ctrl + Shift + T abre una terminal.",
        ],
    ),
    (
        "Bash history",
        [
            "Bash history",
            "Bash 历史命令",
            "Bash 歷史命令",
            "Bash 履歴",
            "Bash 기록",
            "Bash-Verlauf",
            "Historique Bash",
            "Historial Bash",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_fallbacks_and_script_variants() {
        assert_eq!(
            Language::from_locale("zh-Hant-HK"),
            Language::TraditionalChinese
        );
        assert_eq!(Language::from_locale("zh_CN.UTF-8"), Language::Chinese);
        assert_eq!(Language::from_locale("ja-JP"), Language::Japanese);
        assert_eq!(Language::from_locale("de-AT"), Language::German);
        assert_eq!(Language::from_locale("unknown"), Language::English);
    }
    #[test]
    fn catalog_is_complete_and_unique() {
        let mut keys = std::collections::HashSet::new();
        for (key, translations) in STRINGS {
            assert!(keys.insert(key));
            assert!(translations.iter().all(|text| !text.is_empty()));
        }
        assert_eq!(Language::Chinese.t("Settings"), "设置");
    }
}

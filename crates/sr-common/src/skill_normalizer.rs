use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use strsim::damerau_levenshtein;
use unicode_normalization::UnicodeNormalization;

/// スキルエイリアス → 正規形のマッピング（O(1) ルックアップ）
///
/// NOTE: READMEの SKILL_ALIASES と同期すること。
static ALIAS_TO_CANONICAL: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    // NOTE: この辞書は README の SKILL_ALIASES と同期すること（67 canonical skills, 183+ aliases）
    let aliases: &[(&str, &[&str])] = &[
        // JavaScript ecosystem
        (
            "javascript",
            &[
                "js",
                "javascript",
                "java script",
                "ecmascript",
                "es6",
                "es2015",
                "es2016",
                "es2017",
                "es2018",
            ],
        ),
        ("typescript", &["ts", "typescript", "type script"]),
        ("nodejs", &["node.js", "node js", "nodejs", "node"]),
        ("npm", &["npm", "node package manager"]),
        // Frontend frameworks
        (
            "react",
            &[
                "reactjs", "react.js", "react js", "react", "react16", "react17", "react18",
            ],
        ),
        ("vue", &["vue.js", "vuejs", "vue js", "vue", "vue2", "vue3"]),
        (
            "angular",
            &[
                "angularjs",
                "angular.js",
                "angular",
                "angular2",
                "angular4",
                "angular8",
                "angular10",
                "angular12",
            ],
        ),
        ("svelte", &["sveltejs", "svelte.js", "svelte"]),
        ("nextjs", &["next.js", "nextjs", "next js"]),
        ("nuxt", &["nuxtjs", "nuxt.js", "nuxt js", "nuxt"]),
        // CSS and styling
        ("css", &["css", "css3", "cascading style sheets"]),
        (
            "sass",
            &["scss", "sass", "syntactically awesome style sheets"],
        ),
        (
            "bootstrap",
            &["bootstrap", "bootstrap3", "bootstrap4", "bootstrap5"],
        ),
        ("tailwind", &["tailwindcss", "tailwind css", "tailwind"]),
        // Backend frameworks
        (
            "spring",
            &[
                "spring boot",
                "springboot",
                "spring framework",
                "springframework",
                "spring",
            ],
        ),
        (
            "django",
            &["django rest framework", "drf", "django framework", "django"],
        ),
        ("flask", &["flask framework", "python flask", "flask"]),
        (
            "express",
            &[
                "express.js",
                "expressjs",
                "express js",
                "express framework",
                "express",
            ],
        ),
        ("fastapi", &["fast api", "fastapi framework", "fastapi"]),
        ("laravel", &["laravel framework", "php laravel", "laravel"]),
        // Databases
        (
            "postgresql",
            &["postgres", "pg", "postgresql", "postgre sql"],
        ),
        ("mysql", &["my sql", "mysql", "mariadb"]),
        ("mongodb", &["mongo", "mongo db", "mongodb", "nosql"]),
        ("redis", &["redis cache", "redis db", "redis"]),
        ("elasticsearch", &["elastic search", "es", "elasticsearch"]),
        ("sqlite", &["sqlite3", "sql lite", "sqlite"]),
        // Cloud platforms
        (
            "aws",
            &["amazon web services", "amazon aws", "aws cloud", "aws"],
        ),
        ("gcp", &["google cloud platform", "google cloud", "gcp"]),
        (
            "azure",
            &["microsoft azure", "ms azure", "azure cloud", "azure"],
        ),
        (
            "firebase",
            &["google firebase", "firebase platform", "firebase"],
        ),
        // Programming languages
        (
            "python",
            &["python3", "python 3", "py", "python2.7", "python"],
        ),
        (
            "java",
            &[
                "java8",
                "java11",
                "java17",
                "openjdk",
                "oracle java",
                "java",
            ],
        ),
        ("csharp", &["c#", "c sharp", "csharp", ".net", "dotnet"]),
        ("cplusplus", &["c++", "cpp", "c plus plus"]),
        ("golang", &["go", "golang", "go lang"]),
        ("rust", &["rust lang", "rust language", "rust"]),
        ("php", &["php7", "php8", "hypertext preprocessor", "php"]),
        ("ruby", &["ruby lang", "ruby language", "ruby"]),
        ("swift", &["swift lang", "ios swift", "swift"]),
        ("kotlin", &["kotlin lang", "kotlin jvm", "kotlin"]),
        // DevOps and tools
        (
            "docker",
            &["containerization", "docker container", "docker"],
        ),
        (
            "kubernetes",
            &["k8s", "kube", "kubernetes orchestration", "kubernetes"],
        ),
        ("jenkins", &["jenkins ci", "jenkins ci/cd", "jenkins"]),
        (
            "git",
            &["version control", "git scm", "github", "gitlab", "git"],
        ),
        ("terraform", &["infrastructure as code", "iac", "terraform"]),
        ("ansible", &["configuration management", "ansible"]),
        // AI/ML terms (Japanese context)
        (
            "ai",
            &[
                "artificial intelligence",
                "machine learning",
                "ml",
                "人工知能",
                "ai技術",
                "ai",
            ],
        ),
        (
            "ml",
            &[
                "machine learning",
                "artificial intelligence",
                "ai",
                "機械学習",
                "ml",
            ],
        ),
        (
            "llm",
            &[
                "large language model",
                "大規模言語モデル",
                "language model",
                "llm",
            ],
        ),
        (
            "chatgpt",
            &["gpt", "openai", "generative ai", "生成ai", "chatgpt"],
        ),
        (
            "deeplearning",
            &[
                "deep learning",
                "neural networks",
                "ディープラーニング",
                "deeplearning",
            ],
        ),
        ("tensorflow", &["tensor flow", "tf", "tensorflow"]),
        ("pytorch", &["torch", "py torch", "pytorch"]),
        // Testing frameworks
        ("jest", &["jest testing", "jest framework", "jest"]),
        ("cypress", &["cypress testing", "e2e testing", "cypress"]),
        (
            "selenium",
            &["selenium webdriver", "selenium testing", "selenium"],
        ),
        ("junit", &["junit testing", "java testing", "junit"]),
        ("pytest", &["python testing", "py test", "pytest"]),
        // Mobile development
        (
            "reactnative",
            &["react native", "react-native", "rn", "reactnative"],
        ),
        ("flutter", &["flutter framework", "dart flutter", "flutter"]),
        (
            "xamarin",
            &["xamarin forms", "microsoft xamarin", "xamarin"],
        ),
        ("ionic", &["ionic framework", "ionic cordova", "ionic"]),
        // Data and analytics
        ("spark", &["apache spark", "spark streaming", "spark"]),
        ("hadoop", &["apache hadoop", "hadoop ecosystem", "hadoop"]),
        ("kafka", &["apache kafka", "kafka streaming", "kafka"]),
        ("pandas", &["python pandas", "data analysis", "pandas"]),
        ("numpy", &["numerical python", "numpy array", "numpy"]),
    ];

    let mut map = HashMap::new();
    for (canonical, alias_list) in aliases {
        map.insert(*canonical, *canonical);
        for alias in *alias_list {
            map.insert(*alias, *canonical);
        }
    }
    map
});

fn nfkc_lower_trim(input: &str) -> String {
    input.nfkc().collect::<String>().trim().to_lowercase()
}

fn compact_key(input: &str) -> String {
    input
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '　' | '.' | '-' | '_' | '/' | '・' | ','))
        .collect()
}

fn match_canonical_token(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }

    if let Some(canonical) = ALIAS_TO_CANONICAL.get(token) {
        return Some(canonical.to_string());
    }

    let compact = compact_key(token);
    if let Some(canonical) = COMPACT_ALIAS_TO_CANONICAL.get(&compact) {
        return Some((*canonical).to_string());
    }

    fuzzy_match_canonical(&compact)
}

fn split_segments(input: &str) -> impl Iterator<Item = String> + '_ {
    input
        .split(|c: char| matches!(c, ' ' | '　' | '/' | '／' | '・' | ',' | ';' | '|' | '+'))
        .map(nfkc_lower_trim)
        .filter(|s| !s.is_empty())
}

fn fuzzy_match_canonical(compact: &str) -> Option<String> {
    if compact.len() < 4 {
        return None;
    }

    let mut best: Option<(&str, usize)> = None;
    for (alias, canonical) in COMPACT_ALIAS_TO_CANONICAL.iter() {
        // Avoid fuzzy-matching short canonical tokens (e.g., java, rust, go) to
        // reduce false positives on brief or ambiguous inputs. Aliases shorter
        // than 5 characters or canonical targets shorter than 5 are only
        // matched via exact/alias lookups above.
        if alias.len() < 5 || compact.len() < 5 || canonical.len() < 5 {
            continue;
        }

        let distance = damerau_levenshtein(compact, alias);
        if distance == 0 {
            return Some((*canonical).to_string());
        }

        let len = compact.len().max(alias.len());
        let acceptable = distance == 1 || (len >= 8 && distance == 2);
        if !acceptable {
            continue;
        }

        match best {
            None => best = Some((*canonical, distance)),
            Some((_, best_dist)) if distance < best_dist => best = Some((*canonical, distance)),
            _ => {}
        }
    }

    best.map(|(canonical, _)| canonical.to_string())
}

/// 許容しうる軽微な表記揺れに備え、区切り文字除去・NFKC正規化後のキーを持つマップ
static COMPACT_ALIAS_TO_CANONICAL: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    let mut map = HashMap::new();

    for (alias, canonical) in ALIAS_TO_CANONICAL.iter() {
        let compact = compact_key(alias);
        map.entry(compact).or_insert(*canonical);
    }

    map
});

/// スキル文字列を正規形に変換（O(1)）
pub fn normalize_skill(skill: &str) -> String {
    let normalized = nfkc_lower_trim(skill);
    if let Some(canonical) = match_canonical_token(&normalized) {
        return canonical;
    }

    for segment in split_segments(skill) {
        if let Some(canonical) = match_canonical_token(&segment) {
            return canonical;
        }
    }

    normalized
}

/// スキル配列を正規化した HashSet に変換
pub fn normalize_skill_set(skills: &[String]) -> HashSet<String> {
    skills
        .iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| normalize_skill(s))
        .collect()
}

/// スキル配列を正規化した Vec に変換（DB格納用）
pub fn normalize_skills_vec(skills: &[String]) -> Vec<String> {
    let mut result: Vec<String> = skills
        .iter()
        .map(|s| normalize_skill(s))
        .filter(|s| !s.is_empty() && s.len() >= 2)
        .collect();
    result.sort();
    result.dedup();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_alias_equivalence() {
        assert_eq!(normalize_skill("JavaScript"), "javascript");
        assert_eq!(normalize_skill("js"), "javascript");
        assert_eq!(normalize_skill("K8s"), "kubernetes");
        assert_eq!(normalize_skill("C#"), "csharp");
    }

    #[test]
    fn normalizes_fullwidth_and_separators() {
        assert_eq!(normalize_skill("ＡＷＳ"), "aws");
        assert_eq!(normalize_skill("ＧＣＰ"), "gcp");
        assert_eq!(normalize_skill("React　JS"), "react");
        assert_eq!(normalize_skill("Python／Django"), "python");
    }

    #[test]
    fn tolerates_small_typos_for_known_aliases() {
        assert_eq!(normalize_skill("javascirpt"), "javascript");
        assert_eq!(normalize_skill("pytroch"), "pytorch");
        assert_eq!(normalize_skill("kuberntes"), "kubernetes");
    }

    #[test]
    fn does_not_overmatch_short_tokens() {
        assert_eq!(normalize_skill("ab"), "ab");
        assert_eq!(normalize_skill("x"), "x");
    }

    #[test]
    fn does_not_fuzz_short_common_languages() {
        // Guard against false positives on short canonical tokens by requiring
        // lengthier inputs for Damerau–Levenshtein fallback.
        assert_eq!(normalize_skill("javaa"), "javaa");
        assert_eq!(normalize_skill("rustt"), "rustt");
    }

    #[test]
    fn test_unknown_skill_lowercases() {
        assert_eq!(normalize_skill("MyCustomFramework"), "mycustomframework");
    }

    #[test]
    fn test_skill_normalization_bidirectional() {
        let project_skills = vec!["React.js".to_string(), "K8s".to_string()];
        let project_normalized = normalize_skill_set(&project_skills);

        let talent_skills = vec!["react".to_string(), "kubernetes".to_string()];
        let talent_normalized = normalize_skill_set(&talent_skills);

        assert_eq!(project_normalized, talent_normalized);
    }

    #[test]
    fn test_normalize_skills_vec_dedupes_and_sorts() {
        let normalized = normalize_skills_vec(&[
            "Python".to_string(),
            "python".to_string(),
            "  JS ".to_string(),
            "javascript".to_string(),
        ]);

        assert_eq!(
            normalized,
            vec!["javascript".to_string(), "python".to_string()]
        );
    }
}

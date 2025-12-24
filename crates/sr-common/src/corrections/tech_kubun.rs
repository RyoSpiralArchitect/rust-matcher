/// 技術区分ENUMをスキルキーワードから推論
/// 優先順位: 生成AI関連 > 人気技術 > レガシー > None
pub fn infer_tech_kubun(skills: &[String]) -> Option<String> {
    let all_skills = skills.join(" ").to_lowercase();

    let generative_ai_keywords = [
        "生成ai",
        "generative ai",
        "chatgpt",
        "gpt",
        "llm",
        "claude",
        "gemini",
        "openai",
        "llama",
        "llama2",
        "llama3",
        "phi-3",
        "gpt-3.5",
        "gpt-4",
        "mistral",
        "mixtral",
        "grok",
        "perplexity",
        "midjourney",
        "stable diffusion",
        "langchain",
        "llava",
        "whisper",
        "大規模言語モデル",
        "rag",
        "fine-tuning",
        "プロンプト",
        "prompt engineering",
    ];
    if generative_ai_keywords
        .iter()
        .any(|k| all_skills.contains(k))
    {
        return Some("生成AI関連".to_string());
    }

    let popular_keywords = [
        "ai",
        "aws",
        "gcp",
        "azure",
        "ml",
        "機械学習",
        "kubernetes",
        "k8s",
        "docker",
        "helm",
        "argo",
        "pulumi",
        "terraform",
        "ansible",
        "ci/cd",
        "jenkins",
        "github actions",
        "gitlab",
        "datadog",
        "new relic",
        "prometheus",
        "grafana",
        "apache",
        "nginx",
        "mysql",
        "postgres",
        "mongodb",
        "redis",
        "dynamodb",
        "bigquery",
        "snowflake",
        "redshift",
        "databricks",
        "airflow",
        "kafka",
        "pulsar",
        "spark",
        "hive",
        "presto",
        "dbt",
        "react",
        "vue",
        "typescript",
        "javascript",
        "nodejs",
        "node.js",
        "nestjs",
        "next.js",
        "remix",
        "astro",
        "solidjs",
        "nuxt",
        "angular",
        "svelte",
        "flutter",
        "react native",
        "swiftui",
        "jetpack compose",
        "go",
        "rust",
        "python",
        "java",
        "kotlin",
        "scala",
        "c++",
        "matlab",
        "perl",
        "elixir",
        "erlang",
        "haskell",
        "julia",
        "clojure",
        "f#",
        "swift",
        "objective-c",
        "c#",
        "csharp",
        "dotnet",
        ".net",
        "php",
        "laravel",
        "symfony",
        "cakephp",
        "zend",
        "ruby",
        "rails",
        "django",
        "fastapi",
        "spring",
        "spring boot",
        "express",
        "fintech",
        "adtech",
        "edtech",
        "healthtech",
        "hrtech",
        "martech",
        "iot",
        "embedded",
        "devops",
        "sre",
        "データサイエンス",
        "クラウド",
    ];
    if popular_keywords.iter().any(|k| all_skills.contains(k)) {
        return Some("人気技術".to_string());
    }

    let legacy_keywords = [
        "cobol",
        "vb",
        "vb6",
        "vbscript",
        "visual basic",
        "asp",
        "mainframe",
        "メインフレーム",
        "汎用機",
        "as400",
        "rpg",
        "pl/i",
        "fortran",
        "abap",
        "delphi",
        "lotus",
        "lotus notes",
        "notes",
        "domino",
        "foxpro",
        "coldfusion",
        "powerbuilder",
        "access",
    ];
    if legacy_keywords.iter().any(|k| all_skills.contains(k)) {
        return Some("レガシー".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_priority_ordered_kubun() {
        assert_eq!(
            infer_tech_kubun(&["ChatGPT".to_string(), "Python".to_string()]),
            Some("生成AI関連".into())
        );
        assert_eq!(
            infer_tech_kubun(&["Llama2".to_string(), "Java".to_string()]),
            Some("生成AI関連".into())
        );
        assert_eq!(
            infer_tech_kubun(&["AWS".to_string(), "Docker".to_string()]),
            Some("人気技術".into())
        );
        assert_eq!(
            infer_tech_kubun(&["Kotlin".to_string(), "Spring Boot".to_string()]),
            Some("人気技術".into())
        );
        assert_eq!(
            infer_tech_kubun(&["COBOL".to_string(), "AS400".to_string()]),
            Some("レガシー".into())
        );
        assert_eq!(
            infer_tech_kubun(&["Lotus Notes".to_string(), "Delphi".to_string()]),
            Some("レガシー".into())
        );
        assert_eq!(infer_tech_kubun(&["Excel".to_string()]), None);
    }
}

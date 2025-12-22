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
        "mistral",
        "mixtral",
        "grok",
        "perplexity",
        "midjourney",
        "stable diffusion",
        "langchain",
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
        "terraform",
        "ansible",
        "ci/cd",
        "jenkins",
        "github actions",
        "gitlab",
        "apache",
        "nginx",
        "mysql",
        "postgres",
        "mongodb",
        "bigquery",
        "snowflake",
        "spark",
        "react",
        "vue",
        "typescript",
        "javascript",
        "nodejs",
        "node.js",
        "next.js",
        "nuxt",
        "angular",
        "svelte",
        "flutter",
        "react native",
        "go",
        "rust",
        "python",
        "java",
        "kotlin",
        "scala",
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
        "データサイエンス",
        "クラウド",
    ];
    if popular_keywords.iter().any(|k| all_skills.contains(k)) {
        return Some("人気技術".to_string());
    }

    let legacy_keywords = [
        "cobol",
        "vb",
        "visual basic",
        "mainframe",
        "メインフレーム",
        "汎用機",
        "as400",
        "rpg",
        "pl/i",
        "fortran",
        "delphi",
        "lotus",
        "lotus notes",
        "notes",
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

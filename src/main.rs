use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::format;
use std::path::PathBuf;

fn main() -> Result<()> {
    let input_folder = "in";
    let output_folder = "out";

    for folder in std::fs::read_dir(input_folder)?
        .flatten()
        .filter(|x| x.file_type().unwrap().is_dir())
    {
        let meta_path = {
            let mut folder_path = folder.path();
            folder_path.push("meta.toml");
            folder_path
        };


        let meta: CTFMeta = toml::from_str(&std::fs::read_to_string(meta_path)?)?;
        let challenges = meta
            .challenges
            .iter()
            .map(|(a, b)| ((b, a.clone()), a.clone() + ".md"))
            .map(|(a, b)| {
                let mut path = folder.path();
                path.push(b);
                (a, path)
            })
            .flat_map(|(a, b)| Some((a, std::fs::read_to_string(b).ok()?)))
            .collect::<Vec<_>>();

        let front_matter = format!(
            "+++\ntitle=\"{}\"\ndate = {}\n\n[taxonomies]\ntags = [\"ctf-writeups\"]\n+++\n",
            &meta.name, &meta.date
        );
        let description = meta.description.map(|desc| desc + "\n<!-- more -->");

        let section_page = front_matter
            + &description.unwrap_or(String::new())
            + &challenges
                .iter()
                .map(|(_, b)| b.clone())
                .collect::<Vec<_>>()
                .join("\n");

        let challenge_pages = challenges.into_iter().map(|((cmeta, name), content)| {
            let chal_front_matter = format!(
                "+++\ntitle=\"{}\"\ndate = {}\n\n[taxonomies]\ntags = [{}]\n+++\n",
                &cmeta.name,
                &meta.date,
                cmeta
                    .tags
                    .as_ref()
                    .unwrap_or(&vec![])
                    .into_iter()
                    .map(|x| format!("\"{}\"", x))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            ((cmeta, name), chal_front_matter + "\n\n" + &content)
        }).collect::<Vec<_>>();
        let section_path = {
            let mut section_path = PathBuf::new();
            section_path.push(output_folder);
            section_path.push(folder.file_name().to_string_lossy().to_string());
            section_path
        };
        std::fs::create_dir(&section_path);
        let index_path = {
            let mut index_path = section_path.clone();
            index_path.push("index.md");
            index_path
        };
        std::fs::write(index_path, section_page)?;
        for ((cmeta, name), content) in challenge_pages {
            let chal_path = {
                let mut chal_path = section_path.clone();
                chal_path.push(format!("{}.md", name));
                chal_path
            };
            std::fs::write(chal_path, content);
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CTFMeta {
    name: String,
    date: String,
    description: Option<String>,
    challenges: HashMap<String, ChallengeMeta>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChallengeMeta {
    name: String,
    tags: Option<Vec<String>>,
}

mod test {
    use super::*;

    #[test]
    fn parse_meta() {
        let meta = "
name = \"Test!\"

[challenges]
    [challenges.example]
        name = \"Challenge 1\"
";

        let meta: CTFMeta = toml::from_str(meta).unwrap();
        println!("{:?}", meta);
        assert!(false);
    }
}

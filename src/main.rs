use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::format;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy)]
enum OutputType {
    Zola,
    Hugo,
}

impl FromStr for OutputType {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "zola" => Ok(OutputType::Zola),
            "hugo" => Ok(OutputType::Hugo),
            _ => Err("type should be \"zola\" or \"hugo\""),
        }
    }
}

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(short = "i", default_value = "in")]
    input_folder: String,
    #[structopt(short = "o", default_value = "out")]
    output_folder: String,
    #[structopt(short = "t", default_value = "zola")]
    r#type: OutputType,
    #[structopt(short = "a")]
    author: Vec<String>,
}

fn make_front_matter(
    name: impl AsRef<str>,
    date: impl AsRef<str>,
    tags: &[impl AsRef<str>],
    authors: &[impl AsRef<str>],
    output_type: OutputType,
) -> String {
    match output_type {
        OutputType::Zola => format!(
            "+++\ntitle=\"{}\"\ndate = {}\n\n[taxonomies]\ntags = [{}]\n+++\n\n\n",
            name.as_ref(),
            date.as_ref(),
            tags
                .into_iter()
                .map(|x| format!("{:?}", x.as_ref()))
                .collect::<Vec<_>>()
                .join(","),

        ),
        OutputType::Hugo => format!(
            "+++\ntitle=\"{}\"\ndate = {}\ntags = [{}]\nauthors = [{}]\nlayout = \"post\"\n+++\n\n\n",
            name.as_ref(),
            date.as_ref(),
            tags
                .iter()
                .map(|x| format!("{:?}", x.as_ref()))
                .collect::<Vec<_>>()
                .join(","),
            authors
                .iter()
                .map(|x| format!("{:?}", x.as_ref()))
                .collect::<Vec<_>>()
                .join(","),
        ),
    }
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    for folder in std::fs::read_dir(&opt.input_folder)?
        .flatten()
        .filter(|x| x.file_type().unwrap().is_dir())
        .filter(|x| !x.file_name().to_string_lossy().contains(".git"))
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

        let front_matter = make_front_matter(
            &meta.name,
            &meta.date,
            &vec!["ctf-writeups".to_string()],
            &opt.author,
            opt.r#type,
        );
        let description = meta.description.map(|desc| desc + "\n<!-- more -->\n");

        let section_page = front_matter
            + &description.unwrap_or(String::new())
            + &challenges
                .iter()
                .map(|((cmeta, _), b)| format!("# {}\n{}", cmeta.name, b.replace("\n#", "\n##")))
                .collect::<Vec<_>>()
                .join("\n");

        let challenge_pages = challenges.into_iter().map(|((cmeta, name), content)| {
            (
                (cmeta, name),
                format!(
                    "{}{}",
                    make_front_matter(
                        &cmeta.name,
                        &meta.date,
                        &cmeta.tags.as_ref().unwrap_or(&vec![]),
                        &opt.author,
                        opt.r#type
                    ),
                    content
                ),
            )
        });
        let section_path = {
            let mut section_path = PathBuf::new();
            section_path.push(&opt.output_folder);
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
        for ((_, name), content) in challenge_pages {
            let chal_path = {
                let mut chal_path = section_path.clone();
                chal_path.push(format!("{}.md", name));
                chal_path
            };
            std::fs::write(chal_path, content)?;
        }

        let mut assets: Vec<PathBuf> = vec![];
        let mut builder = WalkDir::new(folder.path());

        for entry in builder.into_iter().filter_map(std::result::Result::ok) {
            let entry_path = entry.path();
            if entry_path.is_file() && entry_path.file_name().unwrap() != "meta.toml" {
                match entry_path.extension() {
                    Some(e) => match e.to_str() {
                        Some("md") => continue,
                        _ => assets.push(entry_path.to_path_buf()),
                    },
                    None => assets.push(entry_path.to_path_buf()),
                }
            }
        }
        for asset in assets {
            let relative_path = asset.strip_prefix(folder.path()).unwrap();
            let mut output_path = section_path.clone();
            output_path.push(relative_path);
            std::fs::create_dir_all(output_path.parent().unwrap())?;
            std::fs::copy(asset, output_path)?;
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

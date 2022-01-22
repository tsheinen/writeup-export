use color_eyre::eyre::Result;
use path_dsl::path;
use regex::Regex;
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
    #[structopt(short = "r")]
    // matches URLs in markdown [](.*?) and adds prefix to the url
    rewrite_url_prefix: Option<String>,
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
    process_input_folder(
        &opt.input_folder,
        &opt.output_folder,
        opt.r#type,
        &opt.author,
        opt.rewrite_url_prefix.as_ref(),
    )
}

fn process_input_folder(
    input_folder: &str,
    output_folder: &str,
    output_type: OutputType,
    authors: &[impl AsRef<str>],
    rewrite_url_prefix: Option<impl AsRef<str>>,
) -> Result<()> {
    for folder in std::fs::read_dir(input_folder)?
        .flatten()
        .filter(|x| x.file_type().unwrap().is_dir())
        .filter(|x| !x.file_name().to_string_lossy().contains(".git"))
    {
        let url_regex = Regex::new(r"\[(.*?)\]\(/(.*?)\)").unwrap();
        let top_level_header_regex = Regex::new(r"(?m)^#([^#].*?)$").unwrap();
        let ctf_folder = folder.path();

        let ctf_meta: CTFMeta =
            toml::from_str(&std::fs::read_to_string(path!(&ctf_folder | "meta.toml"))?)?;

        let challenges = ctf_meta
            .challenges
            .iter()
            .map(|(a, b)| ((b, a.clone()), a.clone() + ".md"))
            .map(|(a, b)| (a, path!(&ctf_folder | b)))
            .flat_map(|(a, b)| Some((a, std::fs::read_to_string(b).ok()?)))
            // here we apply transformations on challenge files which should be present in both individual and collected pages
            // 1. if rewrite url prefix is specified, insert into all hrefs
            .map(|(a, content)| {
                if let Some(prefix) = &rewrite_url_prefix {
                    (
                        a,
                        url_regex
                            .replace_all(&content, &format!("[$1](/{}$2)", prefix.as_ref()))
                            .to_string(),
                    )
                } else {
                    (a, content)
                }
            })
            .collect::<Vec<_>>();

        let index_page = {
            let index_front_matter = make_front_matter(
                &ctf_meta.name,
                &ctf_meta.date,
                &vec!["ctf-writeups".to_string()],
                &authors,
                output_type,
            );
            let description = ctf_meta.description.map(|desc| desc + "\n<!-- more -->\n");

            index_front_matter
                + &description.unwrap_or(String::new())
                + &challenges
                    .iter()
                    .map(|((cmeta, name), b)| {
                        format!(
                            "# [{}](/{}/{})\n{}",
                            cmeta.name,
                            folder.file_name().to_string_lossy(),
                            slug::slugify(name),
                            b.replace("\n#", "\n##")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
        };

        let challenge_pages = challenges.into_iter().map(|((cmeta, name), content)| {
            (
                (cmeta, name),
                format!(
                    "{}{}",
                    make_front_matter(
                        &cmeta.name,
                        &ctf_meta.date,
                        &cmeta.tags.as_ref().unwrap_or(&vec![]),
                        authors.as_ref(),
                        output_type
                    ),
                    content
                ),
            )
        });

        let section_path = {
            let mut section_path = PathBuf::from_str(output_folder).unwrap();
            section_path.push(folder.file_name().to_string_lossy().to_string());
            section_path
        };
        std::fs::create_dir(&section_path);

        std::fs::write(path!(&section_path | "index.md"), index_page)?;
        for ((_, name), content) in challenge_pages {
            let chal_md_name = format!("{}.md", name);
            std::fs::write(path!(&section_path | &chal_md_name), content)?;
        }

        let mut assets: Vec<PathBuf> = {
            let mut assets = vec![];
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
            assets
        };

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
    use temp_dir::TempDir;

    #[test]
    fn it_works() -> Result<()> {
        let input_dir = TempDir::new()?;
        let output_dir = TempDir::new()?;
        let ctf_dir = {
            let mut dir = input_dir.path().to_path_buf();
            dir.push("ctf-test");
            dir
        };
        let meta_dir = {
            let mut dir = input_dir.path().to_path_buf();
            dir.push("ctf-test/meta.toml");
            dir
        };
        let md_dir = {
            let mut dir = input_dir.path().to_path_buf();
            dir.push("ctf-test/example.md");
            dir
        };

        let asset_dir = {
            let mut dir = input_dir.path().to_path_buf();
            dir.push("ctf-test/example_asset");
            dir
        };

        std::fs::create_dir_all(&ctf_dir)?;
        std::fs::write(
            &meta_dir,
            "name = \"test lol\"
date = \"2022-01-07\"

[challenges]
[challenges.example]
name = \"example\"
tags = [\"tag 1 lol\"]",
        );
        std::fs::write(&md_dir, "hi lol")?;
        std::fs::write(&asset_dir, "????")?;
        process_input_folder(
            input_dir.path().as_os_str().to_string_lossy().as_ref(),
            output_dir.path().as_os_str().to_string_lossy().as_ref(),
            OutputType::Zola,
            &vec!["sky"],
            None,
        )?;

        let ctf_example_output = {
            let mut dir = output_dir.path().to_path_buf();
            dir.push("ctf-test/example.md");
            std::fs::read_to_string(dir).unwrap()
        };

        let ctf_index_output = {
            let mut dir = output_dir.path().to_path_buf();
            dir.push("ctf-test/index.md");
            std::fs::read_to_string(dir).unwrap()
        };

        let ctf_asset_output = {
            let mut dir = output_dir.path().to_path_buf();
            dir.push("ctf-test/example_asset");
            std::fs::read_to_string(dir).unwrap()
        };

        assert!(std::fs::read_dir(output_dir.path())?
            .filter_map(|x| x.ok())
            .any(|x| x.file_name() == "ctf-test"));
        assert_eq!(
            ctf_example_output,
            "+++
title=\"example\"
date = 2022-01-07

[taxonomies]
tags = [\"tag 1 lol\"]
+++


hi lol"
        );

        assert_eq!(
            ctf_index_output,
            "+++
title=\"test lol\"
date = 2022-01-07

[taxonomies]
tags = [\"ctf-writeups\"]
+++


# example
hi lol"
        );

        assert_eq!(ctf_asset_output, "????");

        Ok(())
    }
}

use rayon::prelude::*;
use std::collections::HashSet;
use std::io::Cursor;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Default, Debug)]
struct AndroidPackage {
    archives: Vec<AndroidArchive>,
    dependencies: Vec<String>,
}

#[derive(Default, Debug)]
struct AndroidArchive {
    host_os: String,
    url: String,
}

fn list_archives<'a>(
    root_url: &str,
    archives_node: &'a roxmltree::Node<'a, 'a>,
) -> Vec<AndroidArchive> {
    let mut packages = vec![];

    for archive in archives_node.children() {
        if archive.has_tag_name("archive") {
            let mut package = AndroidArchive::default();
            for host_os in archive.children() {
                if host_os.has_tag_name("host-os") {
                    package.host_os = host_os.text().unwrap().to_string();
                }
            }

            for complete in archive.children() {
                if complete.has_tag_name("complete") {
                    for url in complete.children() {
                        if url.has_tag_name("url") {
                            package.url = format!("{}{}", root_url, url.text().unwrap());
                            break;
                        }
                    }
                    break;
                }
            }

            packages.push(package);
        }
    }

    packages
}

fn list_dependencies<'a>(dependencies_node: &'a roxmltree::Node<'a, 'a>) -> Vec<String> {
    let mut dependency_paths = vec![];

    for dependency in dependencies_node.children() {
        if dependency.has_tag_name("dependency") {
            dependency_paths.push(dependency.attribute("path").unwrap().to_owned());
        }
    }

    dependency_paths
}

fn find_remote_package_by_name<'a>(
    doc: &'a roxmltree::Document,
    root_url: &str,
    package_name: &str,
) -> AndroidPackage {
    let mut android_package = AndroidPackage::default();

    for dec in doc.descendants() {
        if dec.has_tag_name("remotePackage") && dec.attribute("path") == Some(package_name) {
            for child in dec.children() {
                if child.has_tag_name("archives") {
                    android_package.archives = list_archives(root_url, &child);
                }

                if child.has_tag_name("dependencies") {
                    android_package.dependencies = list_dependencies(&child);
                }
            }

            break;
        }
    }

    android_package
}

fn download_android_sdk_archive(package: &AndroidArchive) -> ZipArchive<Cursor<Box<[u8]>>> {
    let mut response = ureq::get(&package.url).call().unwrap().into_reader();
    let mut data = vec![];
    response.read_to_end(&mut data).unwrap();
    ZipArchive::new(Cursor::new(data.into_boxed_slice())).unwrap()
}

fn recurse_dependency_tree<'a>(
    doc: &roxmltree::Document<'a>,
    root_url: &str,
    package: &str,
    output: &mut HashSet<String>,
) {
    output.insert(package.to_owned());

    let packages = find_remote_package_by_name(doc, root_url, package);
    for dep in packages.dependencies {
        recurse_dependency_tree(doc, root_url, &dep, output);
        output.insert(dep);
    }
}

// instead of the path in the zip file, android sdk expects something slightly more
// elaborate, based on the package name & version
fn androidolize_zipfile_paths(zip_path: &Path, new_roots: &Path) -> PathBuf {
    let mut path_buf = PathBuf::new();
    for (idx, component) in zip_path.components().enumerate() {
        if idx == 0 {
            for root_comp in new_roots.components() {
                path_buf.push(root_comp);
            }
        } else {
            path_buf.push(component)
        }
    }

    path_buf
}

fn is_allowed(path: &Path, allow_list: Option<&[MatchType]>) -> bool {
    if let Some(allow_list) = allow_list {
        for check in allow_list {
            match check {
                MatchType::Partial(check) => {
                    if let Some(file_stem) = path.file_stem() {
                        if file_stem.to_str().unwrap().contains(check) {
                            return true;
                        }
                    }
                }
                MatchType::EntireStem(check) => {
                    if let Some(file_stem) = path.file_stem() {
                        if &file_stem.to_str().unwrap() == check {
                            return true;
                        }
                    }
                }
                MatchType::EntireName(check) => {
                    if let Some(file_stem) = path.file_name() {
                        if &file_stem.to_str().unwrap() == check {
                            return true;
                        }
                    }
                }
                MatchType::EntireFolder(check) => {
                    if let Some(path) = path.to_str() {
                        if path.contains(check) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    } else {
        true
    }
}

const S_IFLNK: u32 = 0o120000; // symbolic link

fn is_symlink(file: &zip::read::ZipFile) -> bool {
    if let Some(mode) = file.unix_mode() {
        if mode & S_IFLNK == S_IFLNK {
            return true;
        }
    }

    false
}

pub fn download_and_extract_packages(
    install_dir: &str,
    host_os: HostOs,
    download_packages: &[&str],
    allow_list: Option<&[MatchType]>,
) {
    let root_url = "https://dl.google.com/android/repository/";
    let packages = ureq::get(&format!("{}/repository2-1.xml", root_url))
        .call()
        .unwrap()
        .into_string()
        .unwrap();

    let doc = roxmltree::Document::parse(&packages).unwrap();

    // make a flat list of all packages and their dependencies
    let mut all_dependencies = HashSet::new();
    for package in download_packages {
        recurse_dependency_tree(&doc, root_url, package, &mut all_dependencies);
    }
    let mut archives = vec![];

    for package_name in all_dependencies {
        let package = find_remote_package_by_name(&doc, root_url, &package_name);

        for archive in package.archives {
            if archive.host_os.contains(host_os.to_str()) || archive.host_os.is_empty() {
                println!("Downloading `{}`", &package_name);
                archives.push((package_name.clone(), archive));
            }
        }
    }

    let mut zip_archives = archives
        .par_iter()
        .map(|(package_name, archive)| (package_name, download_android_sdk_archive(archive)))
        .collect::<Vec<_>>();

    zip_archives
        .par_iter_mut()
        .for_each(|(package_name, zip_archive)| {
            println!("Extracting `{}`", package_name);
            for i in 0..zip_archive.len() {
                let mut file = zip_archive.by_index(i).unwrap();
                let filepath = file.enclosed_name().unwrap();

                let outpath = PathBuf::from(install_dir).join(androidolize_zipfile_paths(
                    filepath,
                    Path::new(&package_name.replace(';', "/")),
                ));

                if is_allowed(filepath, allow_list) {
                    if file.name().ends_with('/') {
                        std::fs::create_dir_all(&outpath).unwrap();
                    } else {
                        if let Some(p) = outpath.parent() {
                            if !p.exists() {
                                std::fs::create_dir_all(&p).unwrap();
                            }
                        }

                        // adapted from https://github.com/zip-rs/zip/issues/77
                        if is_symlink(&file) {
                            let mut contents = Vec::new();
                            file.read_to_end(&mut contents).unwrap();

                            #[cfg(target_family = "unix")]
                            {
                                // Needed to be able to call `OsString::from_vec(Vec<u8>)`
                                use std::os::unix::ffi::OsStringExt as _;

                                let link_path = std::path::PathBuf::from(
                                    std::ffi::OsString::from_vec(contents),
                                );
                                std::os::unix::fs::symlink(link_path, &outpath).unwrap();
                            }

                            #[cfg(target_family = "windows")]
                            {
                                // TODO: Support non-UTF-8 paths (currently only works for paths which are valid UTF-8)
                                let link_path = String::from_utf8(contents).unwrap();
                                std::os::windows::fs::symlink_file(link_path, &outpath).unwrap();
                            }
                        } else {
                            let mut outfile = std::fs::File::create(&outpath).unwrap();
                            std::io::copy(&mut file, &mut outfile).unwrap();

                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Some(mode) = file.unix_mode() {
                                    std::fs::set_permissions(
                                        &outpath,
                                        std::fs::Permissions::from_mode(mode),
                                    )
                                    .unwrap();
                                }
                            }
                        }
                    }
                }
            }
        });
}

pub enum MatchType {
    Partial(&'static str),
    EntireStem(&'static str),
    EntireName(&'static str),
    EntireFolder(&'static str),
}

#[derive(Copy, Clone)]
pub enum HostOs {
    Windows,
    MacOs,
    Linux,
}

impl HostOs {
    fn to_str(self) -> &'static str {
        match self {
            HostOs::Windows => "windows",
            HostOs::Linux => "linux",
            HostOs::MacOs => "macosx",
        }
    }
}

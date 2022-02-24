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

    let packages = find_remote_package_by_name(&doc, root_url, package);
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

fn main() {
    let root_url = "https://dl.google.com/android/repository/";
    let packages = ureq::get(&format!("{}/repository2-1.xml", root_url))
        .call()
        .unwrap()
        .into_string()
        .unwrap();

    let doc = roxmltree::Document::parse(&packages).unwrap();

    let mut all_dependencies = HashSet::new();
    recurse_dependency_tree(&doc, root_url, "ndk;23.1.7779620", &mut all_dependencies);
    recurse_dependency_tree(
        &doc,
        root_url,
        "platforms;android-31",
        &mut all_dependencies,
    );
    recurse_dependency_tree(&doc, root_url, "build-tools;31.0.0", &mut all_dependencies);
    recurse_dependency_tree(&doc, root_url, "platform-tools", &mut all_dependencies);

    let install_dir = "./vendor3/breda-android-sdk/";
    let mut archives = vec![];

    for package_name in all_dependencies {
        let package = find_remote_package_by_name(&doc, root_url, &package_name);

        for archive in package.archives {
            if archive.host_os.contains("windows") || archive.host_os == "" {
                println!("{}", format!("Downloading `{}`", &package_name));
                archives.push((package_name.clone(), archive));
            }
        }
    }

    let mut zip_archives = archives
        .par_iter()
        .map(|(package_name, archive)| (package_name, download_android_sdk_archive(&archive)))
        .collect::<Vec<_>>();

    zip_archives
        .par_iter_mut()
        .for_each(|(package_name, zip_archive)| {
            println!("{}", format!("Extracting `{}`", package_name));
            for i in 0..zip_archive.len() {
                let mut file = zip_archive.by_index(i).unwrap();
                let filepath = file.enclosed_name().unwrap();

                let outpath = PathBuf::from(install_dir).join(androidolize_zipfile_paths(
                    filepath,
                    Path::new(&package_name.replace(";", "/")),
                ));

                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&outpath).unwrap();
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            std::fs::create_dir_all(&p).unwrap();
                        }
                    }

                    let mut outfile = std::fs::File::create(&outpath).unwrap();
                    std::io::copy(&mut file, &mut outfile).unwrap();
                }

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = file.unix_mode() {
                        fs::set_permissions(&outpath, fs::Permissions::from_mode(mode)).unwrap();
                    }
                }
            }
        });
}

// - add aarch64-linux-android to rust-toolchain.toml
// - ANDROID_SDK_ROOT and ANDROID_NDK_ROOT in .config/cargo.toml
// - automatically install cargo-apk

use std::collections::BTreeMap;

use lopdf::{Bookmark, Document, Object, ObjectId};

fn main() {
    let mut arguments: Vec<String> = std::env::args().collect();

    let help_flag: &std::string::String = &String::from("--help");
    let help_flag_sh: &std::string::String = &String::from("--h");

    let credits_flag: &std::string::String = &String::from("--credits");
    let credits_flag_sh: &std::string::String = &String::from("--c");

    let recursive_flag: &std::string::String = &String::from("--recursive");
    let recursive_flag_sh: &std::string::String = &String::from("--r");

    let directory_flag: &std::string::String = &String::from("--dir");
    let directory_flag_sh: &std::string::String = &String::from("--d");

    let mut recursive = false;
    let mut directory = false;

    if arguments.contains(help_flag) || arguments.contains(help_flag_sh) {
        print!("
CombinePDF CLI by Dashiell Rich\n
Usage: ./combine_pdf <destination_path> <pdf1> <pdf2> [<pdf3> ... <pdfN>]\n\n
Arguments:\n
    <destination_path>      Path to the directory where the combined PDF will be placed alongside the name of the output file.\n
    <pdf1>                  Path to the first PDF to combine.\n
    <pdf2>                  Path to the second PDF to combine.\n
    [<pdf3> ... <pdfN>]     (Optional) Paths to any additional PDFs.\n\n
Example Usage:\n
    ./combine_pdf ./output.pdf input1.pdf input2.pdf input3.pdf\n\n
Feature Flags:\n
    --h or --help: Displays usage information alongside a list of feature flags.\n
    --d or --dir: Allows for you to combine a directory of PDFs instead of specifying files.\n
    Note: Usage with --dir flag is as follows: ./combine_pdf [--d | --dir] [--r | --recursive] <destination_path> <source_directory>\n
    --r or --recursive: Can be combined with the directory flag if you would like to combine nested directories of PDFs\n
    --c or --credits: Displays credits alongside the license for this project.
        ");
        return;
    } else if arguments.contains(credits_flag) || arguments.contains(credits_flag_sh) {
        println!("
Created by Dashiell Rich\n
Merge logic by J-F Liu\n
Feel free to do whatever you want with this (according to the MIT license).
        ")
    } else if arguments.len() <= 2 {
        print!(
            "
ERR: Invalid number of arguments\n
Usage: ./combine_pdf <destination_path> <pdf1> <pdf2> [<pdf3> ... <pdfN>]\n\n

Want to see the all of the feature flags?\n
Use --help or --h to have them listed out.
    "
        );
    } else {
        if arguments.contains(recursive_flag) || arguments.contains(recursive_flag_sh) {
            recursive = true;
        }
        if arguments.contains(directory_flag) || arguments.contains(directory_flag_sh) {
            directory = true;
        }
        let num_files = arguments.len();
        let output_path = &arguments.clone()[1];
        let file_names = &mut arguments[2..num_files];
        println!("{:?}", file_names);
        let result = merge_files(file_names, recursive, directory);
        match result {
            Ok(mut document) => {
                document.save(output_path).unwrap();
            }
            Err(err) => {
                eprintln!("{:?}", err);
            }
        }
    }
}

fn merge_files(
    file_names: &mut [String],
    recursive: bool,
    directory: bool,
) -> Result<Document, ()> {
    let mut pdfs_to_merge: Vec<Document> = vec![];

    for file_name in file_names {
        let current_pdf = Document::load(file_name.clone());
        match current_pdf {
            Ok(pdf) => {
                pdfs_to_merge.push(pdf);
                println!("SUCCESS: Added {file_name} to the pdfs_to_merge vector");
            }
            Err(err) => {
                eprintln!("ERROR: Something went wrong.\n{err}")
            }
        }
    }

    // Define a starting `max_id` (will be used as start index for object_ids).
    let mut max_id = 1;
    let mut pagenum = 1;
    // Collect all Documents Objects grouped by a map
    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();
    let mut document = Document::with_version("1.5");

    for mut doc in pdfs_to_merge {
        let mut first = false;
        doc.renumber_objects_with(max_id);

        max_id = doc.max_id + 1;

        documents_pages.extend(
            doc.get_pages()
                .into_iter()
                .map(|(_, object_id)| {
                    if !first {
                        let bookmark = Bookmark::new(
                            String::from(format!("Page_{}", pagenum)),
                            [0.0, 0.0, 1.0],
                            0,
                            object_id,
                        );
                        document.add_bookmark(bookmark, None);
                        first = true;
                        pagenum += 1;
                    }

                    (object_id, doc.get_object(object_id).unwrap().to_owned())
                })
                .collect::<BTreeMap<ObjectId, Object>>(),
        );
        documents_objects.extend(doc.objects);
    }

    // "Catalog" and "Pages" are mandatory.
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    // Process all objects except "Page" type
    for (object_id, object) in documents_objects.iter() {
        // We have to ignore "Page" (as are processed later), "Outlines" and "Outline" objects.
        // All other objects should be collected and inserted into the main Document.
        match object.type_name().unwrap_or("") {
            "Catalog" => {
                // Collect a first "Catalog" object and use it for the future "Pages".
                catalog_object = Some((
                    if let Some((id, _)) = catalog_object {
                        id
                    } else {
                        *object_id
                    },
                    object.clone(),
                ));
            }
            "Pages" => {
                // Collect and update a first "Pages" object and use it for the future "Catalog"
                // We have also to merge all dictionaries of the old and the new "Pages" object
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref object)) = pages_object {
                        if let Ok(old_dictionary) = object.as_dict() {
                            dictionary.extend(old_dictionary);
                        }
                    }

                    pages_object = Some((
                        if let Some((id, _)) = pages_object {
                            id
                        } else {
                            *object_id
                        },
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            "Page" => {}     // Ignored, processed later and separately
            "Outlines" => {} // Ignored, not supported yet
            "Outline" => {}  // Ignored, not supported yet
            _ => {
                document.objects.insert(*object_id, object.clone());
            }
        }
    }

    // If no "Pages" object found, abort.
    if pages_object.is_none() {
        println!("Pages root not found.");

        return Err(());
    }

    // Iterate over all "Page" objects and collect into the parent "Pages" created before
    for (object_id, object) in documents_pages.iter() {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_object.as_ref().unwrap().0);

            document
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    // If no "Catalog" found, abort.
    if catalog_object.is_none() {
        println!("Catalog root not found.");

        return Err(());
    }

    let catalog_object = catalog_object.unwrap();
    let pages_object = pages_object.unwrap();

    // Build a new "Pages" with updated fields
    if let Ok(dictionary) = pages_object.1.as_dict() {
        let mut dictionary = dictionary.clone();

        // Set new pages count
        dictionary.set("Count", documents_pages.len() as u32);

        // Set new "Kids" list (collected from documents pages) for "Pages"
        dictionary.set(
            "Kids",
            documents_pages
                .into_iter()
                .map(|(object_id, _)| Object::Reference(object_id))
                .collect::<Vec<_>>(),
        );

        document
            .objects
            .insert(pages_object.0, Object::Dictionary(dictionary));
    }

    // Build a new "Catalog" with updated fields
    if let Ok(dictionary) = catalog_object.1.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_object.0);
        dictionary.remove(b"Outlines"); // Outlines not supported in merged PDFs

        document
            .objects
            .insert(catalog_object.0, Object::Dictionary(dictionary));
    }

    document.trailer.set("Root", catalog_object.0);

    // Update the max internal ID as wasn't updated before due to direct objects insertion
    document.max_id = document.objects.len() as u32;

    // Reorder all new Document objects
    document.renumber_objects();

    // Set any Bookmarks to the First child if they are not set to a page
    document.adjust_zero_pages();

    // Set all bookmarks to the PDF Object tree then set the Outlines to the Bookmark content map.
    if let Some(n) = document.build_outline() {
        if let Ok(Object::Dictionary(dict)) = document.get_object_mut(catalog_object.0) {
            dict.set("Outlines", Object::Reference(n));
        }
    }

    document.compress();

    return Ok(document);
}

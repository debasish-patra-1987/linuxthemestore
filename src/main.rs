use adw::glib::object::IsA;
use adw::gtk::DrawingArea;
use adw::gtk::SearchEntry;
use adw::prelude::{ActionRowExt, AdwDialogExt, ExpanderRowExt, PreferencesGroupExt};
use chrono::DateTime;
use gtk4::prelude::{ButtonExt, DrawingAreaExt, DrawingAreaExtManual, EditableExt};
use gtk4::{Button, ContentFit, CssProvider, GestureClick, Image, License};
use reqwest::blocking::Client;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;

use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::thread::{self};

use std::fs;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

use adw::gio::prelude::{ApplicationExt, ApplicationExtManual};
use adw::gtk::prelude::{BoxExt, GtkWindowExt, WidgetExt};
use adw::gtk::{
    glib, Align, Box as GtkBox, FlowBox, Label, ListBox, Orientation, Picture, PolicyType,
    ScrolledWindow, SelectionMode,
};
use adw::{
    gdk, AboutDialog, ActionRow, ApplicationWindow, Clamp, ExpanderRow, HeaderBar,
    PreferencesGroup, Spinner, ViewStack,
};
use gtk4::pango::EllipsizeMode;

// Libadwwaita Libraries

pub type Error = std::boxed::Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDetail {
    pub downloadlink: String,
    pub downloadname: String,
    pub downloadsize: u64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductCatalog {
    pub status: String,
    pub statuscode: i64,
    pub message: String,
    pub totalitems: i64,
    pub itemsperpage: i64,
    pub data: Vec<Product>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    pub details: String,
    pub id: i64,
    pub name: String,
    pub typeid: i64,
    pub typename: String,
    pub personid: String,
    pub created: String,
    pub changed: String,
    pub score: f32,
    pub downloads: String,
    pub description: String,
    pub previewpics: Vec<String>,
    pub downloaddetails: Vec<DownloadDetail>,
}

impl<'de> Deserialize<'de> for Product {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Product, <D>::Error>
    where
        D: Deserializer<'de>,
    {
        fn strip_html(source: &str) -> String {
            let mut inside = false;
            source.chars()
                .filter(|&c| {
                    match c {
                        '<' => { inside = true; false },
                        '>' => { inside = false; false },
                        _ => !inside,
                    }
                })
                .collect()
        }

        fn split_field(key: &str) -> Option<(&str, usize)> {
            let digits_start = key.chars().position(|c| c.is_ascii_digit())?;
            let (field, number) = key.split_at(digits_start);
            number.parse().ok().map(|n| (field, n))
        }
        #[derive(Deserialize)]
        struct ProductHelper {
            details: String,
            id: i64,
            name: String,
            //version: String,
            typeid: i64,
            typename: String,
            personid: String,
            created: String,
            changed: String,
            score: f32,
            downloads: String,
            description: String,

            #[serde(flatten)]
            extra: HashMap<String, serde_json::Value>,
        }

        let helper = ProductHelper::deserialize(deserializer)?;
        let mut previewpics = vec![];

        for i in 1..=10 {
            let key = format!("previewpic{}", i);
            if let Some(serde_json::Value::String(url)) = helper.extra.get(&key) {
                previewpics.push(url.clone());
            }
        }

        // Parse numbered download entries into DownloadDetail
        let mut download_map: HashMap<u32, DownloadDetail> = HashMap::new();

        for (key, value) in helper.extra {
            if let Some((field, index)) = split_field(&key) {
                let index = index as u32;
                //

                //let entry = download_map.entry(index);

                let entry = download_map.entry(index).or_insert(DownloadDetail {
                    downloadlink: String::new(),
                    downloadname: String::new(),
                    downloadsize: 0,
                    //downloadmd5sum: String::new(),
                });
                match field {
                    "downloadlink" => {
                        entry.downloadlink = value.as_str().unwrap_or_default().to_string()
                    }
                    "downloadname" => {
                        entry.downloadname = value.as_str().unwrap_or_default().to_string()
                    }
                    "downloadsize" => entry.downloadsize = value.as_u64().unwrap_or(0),
                    //"downloadmd5sum" => entry.downloadmd5sum = value.as_str().unwrap_or_default().to_string(),
                    _ => {}
                }
            }
        }

        let mut downloaddetails: Vec<DownloadDetail> = download_map
            .into_iter()
            .filter(|(_, v)| !v.downloadlink.is_empty())
            .map(|(_, v)| v)
            .collect();

        downloaddetails.sort_by_key(|d| d.downloadname.clone()); // or some other ordering

        Ok(Product {
            details: helper.details,
            id: helper.id,
            name: helper.name,
            //            version: helper.version,
            typeid: helper.typeid,
            typename: helper.typename,
            changed: helper.changed,
            personid: helper.personid,
            created: helper.created,
            score: helper.score / 10.0,
            downloads: match helper.downloads.is_empty() {
                true => "0".to_string(),
                false => helper.downloads,
            },
            description: strip_html(&helper.description),
            previewpics,
            downloaddetails,
        })
    }
}

// Object Types Starts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SortType {
    Latest,
    Rating,
    Creator,
    Downloads,
    Alphabetical,
}
impl SortType {
    pub fn get_label(&self) -> &str {
        match &self {
            SortType::Latest => "update",
            SortType::Rating => "high",
            SortType::Creator => "new",
            SortType::Downloads => "down",
            SortType::Alphabetical => "alpha",
        }
    }
    pub fn to_string(&self) -> &str {
        match &self {
            SortType::Latest => "Latest",
            SortType::Rating => "Rating",
            SortType::Creator => "Creator",
            SortType::Downloads => "Downloads",
            SortType::Alphabetical => "Alphabetical",
        }
    }
    pub fn get_all_sort_types() -> Vec<&'static SortType> {
        vec![
            &SortType::Latest,
            &SortType::Rating,
            &SortType::Creator,
            &SortType::Downloads,
            &SortType::Alphabetical,
        ]
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Catalog {
    FullIconThemes,
    Cursors,
    GnomeShellThemes,
    Gtk4Themes,
}
impl Catalog {
    pub fn get_id(&self) -> &str {
        match &self {
            Catalog::FullIconThemes => "132",
            Catalog::Cursors => "107",
            Catalog::GnomeShellThemes => "134",
            Catalog::Gtk4Themes => "135",
        }
    }
    pub fn to_string(&self) -> &str {
        match &self {
            Catalog::FullIconThemes => "Full Icon Themes",
            Catalog::Cursors => "Cursor Themes",
            Catalog::GnomeShellThemes => "Gnome Shell Themes",
            Catalog::Gtk4Themes => "Gtk Themes",
        }
    }
    pub fn id_to_string(id: &str) -> &str {
        match id {
            "132" => "Full Icon Themes",
            "107" => "Cursor Themes",
            "134" => "Gnome Shell Themes",
            "135" => "Gtk Themes",
            _ => "Others",
        }
    }
    pub fn id_to_catalog(id: &str) -> Catalog {
        match id {
            "132" => Catalog::FullIconThemes,
            "107" => Catalog::Cursors,
            "134" => Catalog::GnomeShellThemes,
            "135" => Catalog::Gtk4Themes,
            _ => Catalog::Gtk4Themes,
        }
    }
    pub fn get_all_catalog_types() -> Vec<Catalog> {
        vec![
            Catalog::FullIconThemes,
            Catalog::Cursors,
            Catalog::GnomeShellThemes,
            Catalog::Gtk4Themes,
        ]
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductPageProps {
    pub pageno: u16,
    pub sortby: SortType,
    pub cat: Catalog,
    pub pagesize: u8,
}

impl Default for ProductPageProps {
    fn default() -> Self {
        //Point { x: 0, y: 0 }
        ProductPageProps {
            pageno: 0,
            sortby: SortType::Latest,
            cat: Catalog::Gtk4Themes,
            pagesize: 10,
        }
    }
}

impl ProductPageProps {
    pub fn set_page(&mut self, pageno: u16) -> &mut ProductPageProps {
        self.pageno = pageno;
        self
    }
    pub fn set_catalog(&mut self, cat: Catalog) -> &mut ProductPageProps {
        self.cat = cat;
        self
    }
    pub fn set_order(&mut self, sortby: SortType) -> &mut ProductPageProps {
        self.sortby = sortby;
        self
    }
    pub fn get_url(&self) -> String {
        //let base_url: Result<String> = get_env_val("BASE_URL");
        let base_url = String::from("www.pling.com");
        /*println!("URL : {}", String::from("https://")
        + &base_url
        + "/ocs/v1/content/data?format=json&pagesize="
        + format!("{}", self.pagesize).as_str()
        + "&categories="
        + self.cat.get_id()
        + "&page="
        + format!("{}", self.pageno).as_str()
        + "&sortmode="
        + self.sortby.get_label());*/
        String::from("https://")
            + &base_url
            + "/ocs/v1/content/data?format=json&pagesize="
            + format!("{}", self.pagesize).as_str()
            + "&categories="
            + self.cat.get_id()
            + "&page="
            + format!("{}", self.pageno).as_str()
            + "&sortmode="
            + self.sortby.get_label()
    }
}

//
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchPageProps {
    pub query: String,
    pub pagesize: u8,
}

impl SearchPageProps {
    fn default(search_text: String) -> Self {
        //Point { x: 0, y: 0 }
        SearchPageProps {
            query: search_text,
            pagesize: 30,
        }
    }

    pub fn set_search_text(&mut self, query: String) -> &mut SearchPageProps {
        self.query = query;
        self
    }
    pub fn get_search_url(&self) -> String {
        //let base_url: Result<String> = get_env_val("BASE_URL");
        let base_url = String::from("www.pling.com");
        String::from("https://")
            + &base_url
            + "/ocs/v1/content/data?format=json&categories=132,107,134,135&pagesize="
            + format!("{}", self.pagesize).as_str()
            + "&page=0"
            + "&sortmode=update"
            + "&search="
            + self.query.as_str()
    }
}

pub fn get_env_val(env_name: &str) -> Result<String> {
    use dotenv::dotenv;
    dotenv().ok();
    Ok(std::env::var(env_name)?)
}
pub fn get_formatted_date(dt: &str) -> String {
    match DateTime::parse_from_rfc3339(dt) {
        Ok(date) => format!("{}", date.format("%d-%m-%Y")),
        Err(e) => format!("{}", e),
    }
}

fn fetch_url(url: &String, file_name: String) -> Result<()> {
    let response = reqwest::blocking::get(url);
    match response {
        Ok(val) => match val.bytes() {
            Ok(content) => {
                let path = std::path::Path::new(&file_name);

                let save_path = &file_name[0..file_name.rfind('/').unwrap()];
                //println!("New Save Dir : {}", save_path);
                let _ = fs::create_dir_all(save_path);

                let mut file = match File::create(&path) {
                    Err(why) => panic!("couldn't create {}", why),
                    Ok(file) => file,
                };
                file.write_all(&content)?;
            }
            Err(e) => {
                panic!("Panic while converting to bytes : {} : {}", url, e);
            }
        },
        Err(e) => {
            panic!("Panic : {}", e);
        }
    }

    Ok(())
}
fn install_theme(downloaddetail: &DownloadDetail, themetype: &Catalog) -> Result<()> {
    let mut path = String::from("/tmp/themedownloadfiles/");
    path.push_str(themetype.to_string());
    path.push_str("/");

    let _ = fs::create_dir_all(&path.as_str());
    path.push_str(&downloaddetail.downloadname);
    match std::path::Path::new(&path).exists() {
        true => {
        }
        false => {
            let _res = fetch_url(&downloaddetail.downloadlink, path.clone());
        }
    }
    let _ = install_tar(
        &path.clone(),
        &themetype,
    )
    .unwrap();
    Ok(())
}

use std::path::PathBuf;
use std::process::Command;

fn install_tar(path: &str, theme_type: &Catalog) -> Result<()> {
    // Construct the target extraction path
    let home_dir = std::env::var("HOME")?;
    let mut extract_path = PathBuf::from(home_dir);

    match theme_type {
        Catalog::FullIconThemes | Catalog::Cursors => {
            extract_path.push(".local/share/icons");
        }
        Catalog::Gtk4Themes | Catalog::GnomeShellThemes => {
            extract_path.push(".local/share/themes");
        }
    }

    fs::create_dir_all(&extract_path)?;

    if path.ends_with(".tar") || path.ends_with(".tar.xz") || path.ends_with(".tar.gz") {
        Command::new("tar")
            .arg("-xf")
            .arg(&path)
            .arg("-C")
            .arg(&extract_path)
            .output()
            .expect("Failed to extract .tar/.tar.xz/.tar.gz");
    } else if path.ends_with(".7z") {
        Command::new("7z")
            .arg("x")
            .arg(&path)
            .arg(format!("-o{}", extract_path.display()))
            .output()
            .expect("Failed to extract .7z");
    } else if path.ends_with(".zip") {
        Command::new("unzip")
            .arg(&path)
            .arg("-d")
            .arg(&extract_path)
            .output()
            .expect("Failed to extract .zip");
    } else {
        println!("Unsupported file type: {}", path);
    }

    Ok(())
}

pub struct CircleRating {
    area: DrawingArea,
    rating: Rc<RefCell<f64>>, // 0.0 to 5.0
}

impl CircleRating {
    pub fn new(rating_value: f64) -> Self {
        let rating = Rc::new(RefCell::new(rating_value.clamp(0.0, 5.0)));
        let area = DrawingArea::new();

        area.set_content_width(250); // 5 circles x 30 px
        area.set_content_height(50); // Height of a circle

        let rating_clone = rating.clone();
        area.set_draw_func(move |_, cr, _, height| {
            let rating = *rating_clone.borrow();
            let circle_diameter = 15.0;
            let spacing = 5.0;

            for i in 0..5 {
                let x = 10.0 + (i as f64 * (circle_diameter + spacing));
                let y = (height as f64 - circle_diameter) / 2.0;

                // Draw the base (empty) circle in light gray
                cr.set_source_rgb(0.8, 0.8, 0.8);
                cr.arc(
                    x + circle_diameter / 2.0,
                    y + circle_diameter / 2.0,
                    circle_diameter / 2.0,
                    0.0,
                    0.0 + std::f64::consts::PI * 2.0,
                );
                cr.stroke().unwrap();

                // Calculate fill level
                let fill_level = (rating - i as f64).clamp(0.0, 1.0);
                if fill_level > 0.0 {
                    cr.save().unwrap();
                    cr.set_source_rgb(0.0, 0.8, 0.0); // Green fill
                    cr.arc(
                        x + circle_diameter / 2.0,
                        y + circle_diameter / 2.0,
                        circle_diameter / 2.0,
                        190.05,
                        190.05 + std::f64::consts::PI * 2.0 * fill_level,
                    );
                    cr.line_to(x + circle_diameter / 2.0, y + circle_diameter / 2.0);
                    cr.fill().unwrap();
                    cr.restore().unwrap();
                }
            }
        });

        Self { area, rating }
    }

    pub fn widget(&self) -> &impl IsA<gtk4::Widget> {
        &self.area
    }

    pub fn set_rating(&self, value: f64) {
        *self.rating.borrow_mut() = value.clamp(0.0, 5.0);
        self.area.queue_draw();
    }
}

pub fn load_custom_css() {
    // Style for smooth round image

    // Load and apply CSS for rounded corners
    let css = CssProvider::new();
    css.load_from_data(
                    "
                    .img-round{
                    border-top-left-radius: 12px;
                    border-top-right-radius: 12px;
                    }

                    .img-cover{
                        border-radius: 2%;
                        border-width: 15px;
                        box-shadow: 0 8px 8px 0 rgba(0, 0, 0, 0.2), 0 6px 8px 0 rgba(0, 0, 0, 0.1);
                    }
                    .blur{
                        filter: blur(1px);
                        -webkit-filter: blur(1px);
                    }
                    .card {
                        box-shadow: 0 8px 8px 0 rgba(0, 0, 0, 0.2), 0 6px 8px 0 rgba(0, 0, 0, 0.1);
                    }
                    button:hover {
                        transform: scale(1.01); /* 120% zoom */
                    }
                    "

                );
    //.unwrap();

    // Apply style to screen
    adw::gtk::style_context_add_provider_for_display(
        &gdk::Display::default().unwrap(),
        &css,
        adw::gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
// Object Types Ends
pub fn get_product_catalog(prodpageprops: &ProductPageProps) -> Result<ProductCatalog> {
    //let mut buffer = Vec::<u8>::new();
    let client = Client::new();
    let url = prodpageprops.get_url();
    let url = url.as_str();
    let res: serde_json::Value = client
        .get(url)
        .send()
        .expect(format!("Invalid Url : {}", url).as_str())
        .json()
        .expect("Failed to get payload");
    //println!("{}", serde_json::to_string_pretty(&res).unwrap());

    let resp_json_products: ProductCatalog = serde_json::from_value(res).unwrap();
    //println!("{}", serde_json::to_string_pretty(&resp_json_products).unwrap());
    Ok(resp_json_products)
}

pub fn get_search_product_catalog(searchpageprops: &SearchPageProps) -> Result<ProductCatalog> {
    //let mut buffer = Vec::<u8>::new();
    let client = Client::new();
    let url = searchpageprops.get_search_url();
    let url = url.as_str();
    println!("Search Url : {}", url);
    let res: serde_json::Value = client
        .get(url)
        .send()
        .expect(format!("Invalid Url : {}", url).as_str())
        .json()
        .expect("Failed to get payload");
    //println!("{}", serde_json::to_string_pretty(&res).unwrap());

    let resp_json_products: ProductCatalog = serde_json::from_value(res).unwrap();
    println!(
        "{}",
        serde_json::to_string_pretty(&resp_json_products).unwrap()
    );
    Ok(resp_json_products)
}

fn downloadthumb(each_product: &Product) -> Result<()> {
    //println!("Got inside Download Thumbnail");

    let firstimage = match each_product.previewpics.len() {
        0 => None,
        _ => Some(&each_product.previewpics[0]),
    };
    if firstimage.is_none() {
        return Ok(());
    }
    let firstimage = firstimage.unwrap();
    let save_path = "/tmp/themeinstaller/cache/".to_string() + &firstimage;
    if !std::path::Path::new(&save_path).exists() {
        let mut save_dir = save_path.to_string();
        save_dir.push_str(&firstimage);

        let save_dir = &save_dir[0..save_dir.rfind('/').unwrap()];
        let save_dir_copy = save_dir;
        let _ = fs::create_dir_all(&save_dir_copy);

        fetch_url(&firstimage.replace("770x540", "770x540"), save_path).unwrap();
    }

    Ok(())

    //);
}

fn downloadotherimages(each_product: &Product) -> Result<()> {
    //println!("Got inside Download Thumbnail");

    for each_image in &each_product.previewpics[1..] {
        let save_path = "/tmp/themeinstaller/cache/".to_string() + &each_image;
        if !std::path::Path::new(&save_path).exists() {
            let mut save_dir = save_path.to_string();
            save_dir.push_str(&each_image);

            let save_dir = &save_dir[0..save_dir.rfind('/').unwrap()];
            let save_dir_copy = save_dir;
            let _ = fs::create_dir_all(&save_dir_copy);

            fetch_url(&each_image.replace("770x540", "770x540"), save_path).unwrap();
        }
    }
    Ok(())

    //);
}

fn _downloadthumbs(products: Vec<Product>) -> Result<()> {
    //println!("Got inside Download Thumbnail");

    let mut handles = vec![];

    for each_product in products {
        //let each_image = if let Some(each_image) = each_product.previewpics[0];
        let imagelist = match each_product.previewpics.len() {
            0 => None,
            _ => Some(each_product.previewpics.clone()),
        };
        let counter = Arc::new(Mutex::new(imagelist));
        let handle = thread::spawn(move || {
            let image_small_mutex = counter.lock().unwrap();
            let image_small_list = image_small_mutex.as_ref().unwrap();
            //println!("Image link : {:?}", image_small.clone().unwrap());
            //println!("In async tokio");
            for image_small in image_small_list {
                let save_path = "/tmp/themeinstaller/cache/".to_string() + &image_small;
                if !std::path::Path::new(&save_path).exists() {
                    let mut save_dir = save_path.to_string();
                    save_dir.push_str(&image_small);

                    let save_dir = &save_dir[0..save_dir.rfind('/').unwrap()];
                    let save_dir_copy = save_dir;
                    let _ = fs::create_dir_all(&save_dir_copy);

                    fetch_url(&image_small.replace("770x540", "770x540"), save_path).unwrap();
                }
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.join().unwrap();
    }
    Ok(())

    //);
}

fn build_category_page(
    view_stack: &ViewStack,
    outer_view_stack: &GtkBox,
    theme_type: &Catalog,
    window: &ApplicationWindow,
) {
    let themecategoryloadingpage = GtkBox::new(Orientation::Vertical, 10);
    themecategoryloadingpage.add_css_class("background");
    let _themecategorypage_viewstack = view_stack.add_titled(
        &themecategoryloadingpage,
        Some(theme_type.to_string()),
        theme_type.to_string(),
    );

    let themecategorysortbybutton = adw::InlineViewSwitcher::new();
    //themecategorysortbybutton.add_css_class("round");
    themecategorysortbybutton.set_can_shrink(true);

    let themecategorysortby_view_stack = adw::ViewStack::new();
    themecategorysortby_view_stack.set_enable_transitions(true);
    //themecategorysortby_view_stack.add_css_class("background");
    themecategorysortby_view_stack.set_transition_duration(20);

    themecategorysortbybutton.set_valign(Align::Start);
    themecategorysortbybutton.set_halign(Align::Center);
    themecategoryloadingpage.append(&themecategorysortbybutton);
    //outer_view_stack.append(&fulliconsortbybutton);

    outer_view_stack.append(&themecategorysortbybutton);

    themecategorysortbybutton.set_stack(Some(&themecategorysortby_view_stack));
    let themecategorysortby_view_stack_box = GtkBox::new(Orientation::Vertical, 0);
    themecategorysortby_view_stack_box.append(&themecategorysortby_view_stack);
    themecategoryloadingpage.append(&themecategorysortby_view_stack_box);

    // Initial Screen Widgets below Ends

    // Starting async loading of items for each page
    // fullcionprodpage

    for each_sorting_type in SortType::get_all_sort_types() {
        build_content_box(
            ProductPageProps::default()
                .set_catalog(theme_type.to_owned())
                .set_order(each_sorting_type.to_owned()),
            &themecategorysortby_view_stack,
            &window,
        );
    }
}

fn build_search_page(
    view_stack: &ViewStack,
    outer_view_stack: &GtkBox,
    window: &ApplicationWindow,
) {
    let searchbox = GtkBox::new(Orientation::Vertical, 10);
    searchbox.add_css_class("background");
    let _searchpage_viewstack =
        view_stack.add_titled(&searchbox, Some("Search Themes"), "Search Themes");

    let searchinput = SearchEntry::new();
    searchinput.set_search_delay(500);
    searchinput.set_placeholder_text(Some("e.g. Papirus Theme"));

    //searchinput.add_css_class("round");

    searchbox.set_valign(Align::Fill);
    searchbox.set_halign(Align::Fill);
    searchbox.set_homogeneous(false);

    let searchinputbox = GtkBox::new(Orientation::Vertical, 10);
    searchinputbox.set_valign(Align::Start);
    searchinputbox.set_halign(Align::Center);
    searchinputbox.set_homogeneous(false);

    searchinputbox.append(&searchinput);
    searchbox.append(&searchinputbox);

    outer_view_stack.append(&searchbox);
    let window_clone = window.clone();

    //create_search_page(&search_text, &searchresultpage);
    let searchresultpage = GtkBox::new(Orientation::Vertical, 10);
    searchresultpage.set_valign(Align::Fill);
    searchresultpage.set_halign(Align::Fill);
    searchresultpage.set_vexpand(true);
    searchresultpage.set_hexpand(true);

    //let searchpageprops = SearchPageProps::default("".to_owned());
    build_search_content_box(&searchinput, &searchresultpage, &window_clone);

    searchbox.append(&searchresultpage);
    println!("{}", searchinput.text().to_string());

    //    let searchpageprops = SearchPageProps::default(searchinput.text().to_string() );
}
// contentbox function
fn build_flowbox_for_page(each_product: &Product, flowbox: &FlowBox, window: &ApplicationWindow) {
    let imgpath = "/tmp/themeinstaller/cache/".to_string() + &each_product.previewpics[0];
    let img = Picture::builder()
        .valign(Align::Center)
        .hexpand_set(false)
        .vexpand_set(false)
        .margin_start(0)
        .margin_end(0)
        .margin_top(0)
        .margin_bottom(0)
        .css_name("img-cover")
        .build();
    img.add_css_class("img-round");
    img.set_content_fit(ContentFit::Cover);
    //img.set_filename(Some(&std::path::Path::new(imgpath.as_str())));
    img.set_size_request(260, 260);
    //img.set_can_shrink(true);
    let imgclamp = Clamp::new();
    let imagespinner = Spinner::builder()
        .valign(Align::Center)
        .halign(Align::Center)
        .hexpand(true)
        .vexpand(true)
        .width_request(32)
        .height_request(32)
        .build();
    let imagebox = GtkBox::builder()
        .valign(Align::Center)
        .halign(Align::Center)
        .hexpand(true)
        .vexpand(true)
        .height_request(260)
        .width_request(260)
        .build();
    imagebox.append(&imagespinner);
    //imgclamp.set_child(Some(&img));
    //println!("Setting SPinner");
    imgclamp.set_child(Some(&imagebox));
    imgclamp.set_tightening_threshold(256);
    imgclamp.set_maximum_size(256);

    let flowboxchild_button = Button::builder()
        .width_request(256)
        .css_classes(vec!["flat"])
        .build();
    let flowboxchild = GtkBox::builder()
        .hexpand_set(true)
        .vexpand_set(true)
        .orientation(Orientation::Vertical)
        .valign(Align::End)
        .halign(Align::End)
        .margin_start(10)
        .margin_end(10)
        .margin_top(5)
        .margin_bottom(10)
        .css_classes(vec!["card", "activable1"])
        .build();
    flowboxchild_button.set_child(Some(&flowboxchild));

    let productclamp = Clamp::builder().build();
    productclamp.set_valign(Align::Start);
    //productclamp.set_widget_name(&each_product.id.to_string().as_str());
    productclamp.set_maximum_size(256);
    productclamp.set_child(Some(&flowboxchild_button));
    flowboxchild.append(&imgclamp);

    let prodnametype_holder = GtkBox::new(Orientation::Vertical, 0);
    prodnametype_holder.set_margin_bottom(10);
    prodnametype_holder.set_margin_top(0);
    prodnametype_holder.set_margin_start(0);
    prodnametype_holder.set_margin_end(10);
    prodnametype_holder.set_valign(Align::End);
    prodnametype_holder.set_halign(Align::End);

    prodnametype_holder.append(
        &Label::builder()
            .label(&each_product.name)
            .ellipsize(EllipsizeMode::End)
            .margin_bottom(0)
            .margin_top(5)
            .margin_start(10)
            .margin_end(10)
            .css_classes(vec!["heading", "accent"])
            .halign(Align::Start)
            .valign(Align::Center)
            .hexpand_set(true)
            .vexpand_set(true)
            .build(),
    );
    prodnametype_holder.append(
        &Label::builder()
            .label(Catalog::id_to_string(&each_product.typeid.to_string()))
            .margin_bottom(0)
            .margin_top(0)
            .margin_start(10)
            .margin_end(10)
            .ellipsize(EllipsizeMode::End)
            .css_classes(vec!["caption", "dimmed"])
            .halign(Align::Start)
            .valign(Align::Center)
            .hexpand_set(true)
            .vexpand_set(true)
            .build(),
    );

    flowboxchild.append(&prodnametype_holder);

    let lastbox = GtkBox::new(Orientation::Vertical, 0);
    prodnametype_holder.append(&lastbox);
    //lastbox.set_homogeneous(true);
    lastbox.set_hexpand(true);
    lastbox.set_vexpand(true);
    lastbox.set_halign(Align::Fill);
    lastbox.set_valign(Align::End);

    let lastbox0 = GtkBox::new(Orientation::Horizontal, 0);
    lastbox0.set_css_classes(&vec!["flat"]);
    lastbox0.set_hexpand(true);
    lastbox0.set_vexpand(true);
    lastbox0.set_halign(Align::Start);
    lastbox0.set_valign(Align::End);
    //lastbox1.set_css_classes(&vec!["card"]);
    lastbox0.set_homogeneous(true);
    lastbox.append(&lastbox0);
    lastbox0.set_halign(Align::Fill);

    let lastbox1 = GtkBox::new(Orientation::Horizontal, 0);
    lastbox1.set_css_classes(&vec!["flat"]);
    //lastbox1.set_css_classes(&vec!["card"]);
    lastbox1.set_homogeneous(true);
    lastbox1.set_valign(Align::Start);
    lastbox1.set_halign(Align::Start);
    lastbox1.set_hexpand(true);
    lastbox1.set_vexpand(true);
    let lastbox2 = GtkBox::new(Orientation::Horizontal, 0);
    //lastbox2.set_css_classes(&vec!["card"]);
    lastbox1.set_css_classes(&vec!["flat"]);
    lastbox2.set_homogeneous(true);
    //lastbox2.set_valign(Align::End);
    lastbox2.set_hexpand(true);
    lastbox2.set_vexpand(true);

    let lastbox3 = GtkBox::new(Orientation::Horizontal, 0);
    //lastbox2.set_css_classes(&vec!["card"]);
    lastbox3.set_homogeneous(true);
    lastbox3.set_valign(Align::End);
    lastbox3.set_hexpand(true);
    lastbox3.set_vexpand(true);

    lastbox.append(&lastbox1);
    lastbox.append(&lastbox2);
    lastbox.append(&lastbox3);
    lastbox.set_valign(Align::End);

    //lastbox.set_valign(Align::Center);
    lastbox.set_halign(Align::Start);
    lastbox.set_vexpand(false);
    lastbox.set_hexpand(false);
    //flowboxchild.append(&lastbox);

    //lastbox1.append(&ActionRow::builder().subtitle(Catalog::id_to_string(&each_product.typeid.to_string().as_str())).title("Product Type").activatable(false).build());

    let rating_widget = CircleRating::new(((each_product.score / 2.0) as f32).into());

    lastbox0.append(rating_widget.widget());
    //lastbox1.append(&ActionRow::builder().subtitle(String::from(each_product.score.to_string())+"/10").title("Score").activatable(false).build());
    lastbox2.append(
        &ActionRow::builder()
            .subtitle(&each_product.downloads)
            .title("Downloads")
            .activatable(false)
            .build(),
    );
    lastbox2.append(
        &ActionRow::builder()
            .subtitle(get_formatted_date(&each_product.changed))
            .title("Last Updated")
            .activatable(false)
            .build(),
    );
    lastbox3.append(
        &ActionRow::builder()
            .subtitle(&each_product.personid)
            .title("User")
            .activatable(false)
            .subtitle_lines(1)
            .build(),
    );
    lastbox3.append(
        &ActionRow::builder()
            .subtitle(get_formatted_date(&each_product.created))
            .title("Created")
            .activatable(false)
            .build(),
    );

    let lastbox4 = GtkBox::new(Orientation::Horizontal, 0);
    lastbox.append(&lastbox4);
    //lastbox2.set_css_classes(&vec!["card"]);
    lastbox4.set_homogeneous(true);
    lastbox4.set_halign(Align::Fill);
    lastbox4.set_valign(Align::Fill);
    lastbox4.set_hexpand(true);
    lastbox4.set_vexpand(true);

    // Starts

    let (imagesender, imagerecv) = async_channel::unbounded::<String>();

    let each_prod_clone = each_product.clone();
    let each_prod_clone_arc = Arc::new(Mutex::new(each_prod_clone));
    imagespinner.connect_realize(move |_| {
        //eprintln!("Inside Image Spinner connect realize!");

        let sender = imagesender.clone();
        let each_prod_clone_arc = Arc::clone(&each_prod_clone_arc);

        // Run async code to get all required values for populating full icon themes
        adw::gio::spawn_blocking(move || {
            let each_prod_clone_mutex = each_prod_clone_arc.lock().unwrap();
            let each_prod_clone = each_prod_clone_mutex.deref();
            //println!("Before download : {:#?}", &each_prod_clone);

            let _ = downloadthumb(&each_prod_clone);
            //println!("After download : {:#?}", &each_prod_clone);
            sender
                .send_blocking(String::from("imgcomplete"))
                .unwrap_or_default();
            //println!("After sending");
            downloadotherimages(&each_prod_clone).unwrap_or_default();
        });

        // The main loop executes the asynchronous block
        let imagerecv_clone = imagerecv.clone();
        let imgclamp_clone = imgclamp.clone();
        let imgclone = img.clone();
        let imgpath_clone = imgpath.clone();
        glib::spawn_future_local({
            async move {
                while let Ok(message) = imagerecv_clone.recv().await {
                    if message.eq(&String::from("imgcomplete")) {
                        imgclone.set_filename(Some(&std::path::Path::new(imgpath_clone.as_str())));
                        imgclamp_clone.set_child(Some(&imgclone));
                        //println!("Set the image after download")
                    } else {
                    }
                }
            }
        });
    });

    // Ends

    flowbox.insert(&productclamp, -1);

    let window_clone = window.clone();
    let product = each_product.clone();
    let ges_click = GestureClick::new();
    flowboxchild.add_controller(ges_click.clone());
    //ges_click.connect_pressed(move |_, _, _, _| {
    flowboxchild_button.connect_clicked(move |_| {
        //println!("clicked");

        let dialog = adw::PreferencesDialog::builder()
            .can_close(true)
            .presentation_mode(adw::DialogPresentationMode::Floating)
            .build();

        let dialogbox = GtkBox::builder()
            .spacing(10)
            .orientation(Orientation::Vertical)
            .vexpand(true)
            .hexpand(true)
            .build();

        let dialogheader = HeaderBar::builder().build();
        dialogbox.append(&dialogheader);
        dialogheader.set_css_classes(&vec!["background"]);
        //dialogheader.set_show_back_button(true);
        let header_title =
            adw::WindowTitle::new(&product.name, "Select the variants to install below");
        dialogheader.set_title_widget(Some(&header_title));

        let dialog_scrollbox = ScrolledWindow::builder()
            .propagate_natural_height(true)
            .propagate_natural_width(true)
            .hscrollbar_policy(PolicyType::Automatic)
            .margin_bottom(10)
            .margin_end(10)
            .margin_top(10)
            .margin_start(10)
            .vscrollbar_policy(PolicyType::Automatic)
            .build();

        let dialogbody = GtkBox::new(Orientation::Vertical, 0);
        dialog_scrollbox.set_child(Some(&dialogbody));
        dialogbox.append(&dialog_scrollbox);

        //Insert Images in dialog body
        let total_preview_pics = product.previewpics.len();
        let imgpath = "/tmp/themeinstaller/cache/".to_string() + &product.previewpics[0];
        let img = Picture::builder()
            .valign(Align::Center)
            .hexpand_set(true)
            .vexpand_set(true)
            .margin_start(0)
            .margin_end(0)
            .margin_top(0)
            .margin_bottom(0)
            .css_name("img-cover")
            .build();
        img.add_css_class("img-cover");
        img.set_size_request(512, 512);
        img.set_content_fit(ContentFit::Cover);
        img.set_filename(Some(&std::path::Path::new(imgpath.as_str())));

        let each_img_box = GtkBox::builder()
            .spacing(10)
            .orientation(Orientation::Horizontal)
            .vexpand(false)
            .hexpand(false)
            .build();
        let prev_button = Button::builder()
            .icon_name("go-previous-symbolic")
            .css_classes(vec!["circular"])
            .hexpand(true)
            .vexpand(true)
            .halign(Align::Center)
            .valign(Align::Center)
            .margin_bottom(15)
            .margin_top(0)
            .build();

        let next_button = Button::builder()
            .icon_name("go-next-symbolic")
            .css_classes(vec!["circular"])
            .hexpand(true)
            .vexpand(true)
            .halign(Align::Center)
            .valign(Align::Center)
            .margin_bottom(15)
            .margin_top(0)
            .build();
        each_img_box.append(&prev_button);
        each_img_box.append(&img);
        each_img_box.append(&next_button);

        let imgclamp = Clamp::new();
        imgclamp.set_css_classes(&vec!["clamp"]);
        imgclamp.set_child(Some(&each_img_box));
        imgclamp.set_tightening_threshold(256);
        imgclamp.set_maximum_size(256);
        imgclamp.set_margin_top(20);
        imgclamp.set_margin_bottom(20);

        let current_index = Arc::new(Mutex::new((0, total_preview_pics)));
        let previewpics = product.previewpics.clone();
        let img_prev = img.clone();
        prev_button.connect_clicked(move |_prev_button| {
            let mut curret_index_mutex = current_index.lock().unwrap();
            let (current_index, total_preview_pics) = curret_index_mutex.deref_mut();
            if *current_index == 0 {
                *current_index = *total_preview_pics - 1;
            } else {
                *current_index -= 1;
            }
            let current_index = *current_index as usize;
            let imgpath = "/tmp/themeinstaller/cache/".to_string() + &previewpics[current_index];
            img_prev.set_filename(Some(&std::path::Path::new(imgpath.as_str())));
        });

        let current_index = Arc::new(Mutex::new((0, total_preview_pics as i32)));
        let previewpics_next = product.previewpics.clone();
        let img_next = img.clone();
        next_button.connect_clicked(move |_next_button| {
            let mut curret_index_mutex = current_index.lock().unwrap();
            let (current_index, total_preview_pics) = curret_index_mutex.deref_mut();
            if *current_index == (*total_preview_pics - 1) {
                *current_index = 0;
            } else {
                *current_index += 1;
            }
            let current_index = *current_index as usize;
            let imgpath =
                "/tmp/themeinstaller/cache/".to_string() + &previewpics_next[current_index];
            img_next.set_filename(Some(&std::path::Path::new(imgpath.as_str())));
        });

        dialogbody.append(&imgclamp);

        dialog.set_child(Some(&dialogbox));

        let group = PreferencesGroup::builder()
            .title("Select Variants to Download")
            .build();

        for each_variant in &product.downloaddetails {
            let downloadsize_in_mb =
                ((each_variant.downloadsize as f32) / 100.0).to_string() + " Mb";
            let row: ActionRow = ActionRow::builder()
                .activatable(false)
                .title(&each_variant.downloadname)
                .subtitle(downloadsize_in_mb)
                //.css_name("card")
                .build();
            let downloadbutton = Button::builder()
                .css_classes(vec!["pill1"])
                .icon_name("document-save-symbolic")
                .margin_bottom(10)
                .margin_top(10)
                .sensitive(true)
                .build();
            row.add_suffix(&downloadbutton);
            let new_variant = each_variant.clone();
            let catalogtype = Catalog::id_to_catalog(&product.typeid.to_string().as_str());

            let (senderdownload, receiverdownload) = async_channel::unbounded::<String>();
            downloadbutton.connect_clicked(move |downloadbutton| {
                downloadbutton.set_child(Some(&Spinner::new()));
                eprintln!("Clicked!");

                let sender = senderdownload.clone();
                let catalogtype_arc = Arc::new(Mutex::new(catalogtype.clone()));
                let new_variant_clone = new_variant.clone();
                // Run async code to get all required values for populating full icon themes
                adw::gio::spawn_blocking(move || {
                    let catalogtype_mutex = catalogtype_arc.lock().unwrap();
                    let catalogtype = catalogtype_mutex.deref();
                    let _ = install_theme(&new_variant_clone, &catalogtype);
                    sender
                        .send_blocking("downloaded".to_string())
                        .unwrap_or_default();
                });

                // The main loop executes the asynchronous block
                let receiverdownload_clone = receiverdownload.clone();
                let downloadbutton_clone = downloadbutton.clone();
                glib::spawn_future_local({
                    async move {
                        while let Ok(message) = receiverdownload_clone.recv().await {
                            if message.eq(&String::from("downloaded")) {
                                downloadbutton_clone.set_icon_name("ephy-download-done-symbolic");
                                downloadbutton_clone.set_sensitive(false);
                            } else {
                            }
                        }
                    }
                });
            });
            group.add(&row);
        }
        // Add the ListBox to the dialog
        let productbox = GtkBox::new(Orientation::Vertical, 5);
        dialogbody.append(&productbox);

        let productlistbox = GtkBox::builder()
            .width_request(500)
            .orientation(Orientation::Vertical)
            .halign(Align::Fill)
            .build();

        let productlistrow = GtkBox::builder()
            .width_request(500)
            .orientation(Orientation::Horizontal)
            .halign(Align::Fill)
            .css_classes(vec!["card"])
            .build();

        productlistrow.append(
            &ActionRow::builder()
                .activatable(false)
                .title("Product Name")
                .subtitle(&product.name)
                .halign(Align::Start)
                .build(),
        );

        productlistrow.append(
            &ActionRow::builder()
                .activatable(false)
                .title("Theme Type")
                .halign(Align::End)
                .subtitle(&product.typename)
                .build(),
        );

        productlistbox.append(&productlistrow);

        let productlistrow = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .halign(Align::Baseline)
            .width_request(500)
            .css_classes(vec!["card"])
            .build();

        productlistrow.append(
            &ActionRow::builder()
                .activatable(false)
                .title("Downloads")
                .halign(Align::Baseline)
                .subtitle(&product.downloads)
                .build(),
        );

        productlistrow.append(
            &ActionRow::builder()
                .activatable(false)
                .title("Updated By")
                .halign(Align::Baseline)
                .subtitle(&product.personid)
                .build(),
        );

        productlistbox.append(&productlistrow);

        let productlistrow = GtkBox::builder()
            .width_request(500)
            .orientation(Orientation::Horizontal)
            .halign(Align::Fill)
            .css_classes(vec!["card"])
            .build();

        productlistrow.append(
            &ActionRow::builder()
                .activatable(false)
                .title("Created On")
                .halign(Align::Start)
                .subtitle(get_formatted_date(&product.created))
                .build(),
        );

        productlistrow.append(
            &ActionRow::builder()
                .activatable(false)
                .title("Updated On")
                .halign(Align::Fill)
                .subtitle(get_formatted_date(&product.changed))
                .build(),
        );

        productlistbox.append(&productlistrow);

        let descriptionrow = ExpanderRow::builder()
            .title("Description")
            .subtitle("Show more")
            .activatable(false)
            .margin_top(0)
            .margin_end(0)
            .margin_bottom(0)
            .margin_start(0)
            .width_request(500)
            .build();

        // Content to show when expanded
        let expander_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .margin_bottom(5)
            .margin_end(5)
            .margin_top(5)
            .margin_start(10)
            .build();

        expander_box.append(
            &Label::builder()
                .label(&product.description)
                .wrap(true)
                .css_classes(vec!["caption", "dimmed"])
                .build(),
        );

        descriptionrow.add_row(&expander_box);

        let descriptionlistrow = ListBox::builder()
            .margin_top(32)
            .margin_end(32)
            .margin_bottom(32)
            .margin_start(32)
            .selection_mode(SelectionMode::None)
            .css_classes(vec![String::from("boxed-list")])
            .build();

        descriptionlistrow.append(&descriptionrow);

        productbox.append(
            &adw::Clamp::builder()
                .child(&group)
                .maximum_size(500)
                .build(),
        );
        productbox.append(
            &adw::Clamp::builder()
                .child(&descriptionlistrow)
                .maximum_size(500)
                .build(),
        );
        dialog.present(Some(&window_clone));
    });
}
fn build_content_box(
    productpage: &ProductPageProps,
    themecategorysortby_view_stack: &ViewStack,
    window: &ApplicationWindow,
) {
    let themecategory_contentbox = GtkBox::new(Orientation::Vertical, 20);
    //window.set_height_request(1024);
    themecategory_contentbox.set_valign(Align::Center);
    themecategory_contentbox.set_halign(Align::Center);

    themecategory_contentbox.set_vexpand(true);
    themecategory_contentbox.set_hexpand(true);
    let spinner_loading_themecategory_latest = Spinner::new();
    spinner_loading_themecategory_latest.set_width_request(48);
    spinner_loading_themecategory_latest.set_height_request(48);

    let spinner_label_themecategory_latest = Label::builder()
        .label(String::from("Fetching ") + productpage.cat.to_string() + ". Please wait...")
        .css_classes(vec!["dimmed", "Heading-4"])
        .build();

    themecategory_contentbox.append(&spinner_loading_themecategory_latest);
    themecategory_contentbox.append(&spinner_label_themecategory_latest);
    let themecategory_loadingpage = GtkBox::new(Orientation::Vertical, 0);
    themecategory_loadingpage.append(&themecategory_contentbox);

    let _fulliconpage_viewstack_latest = themecategorysortby_view_stack.add_titled(
        &themecategory_loadingpage,
        Some(productpage.sortby.to_string()),
        productpage.sortby.to_string(),
    );
    //_page async loading of first page

    let contentpage = GtkBox::new(Orientation::Vertical, 0);
    //contentpage.set_css_classes(&vec!["card"]);

    let flowbox = FlowBox::builder().build();
    flowbox.set_vexpand(true);
    flowbox.set_hexpand(true);
    flowbox.set_valign(Align::Center);
    flowbox.set_halign(Align::Center);
    flowbox.set_activate_on_single_click(false);
    flowbox.set_min_children_per_line(1);
    flowbox.set_max_children_per_line(5);
    flowbox.set_selection_mode(SelectionMode::None);
    flowbox.set_css_classes(&vec!["suggested-action"]);

    let scrollwindow = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    scrollwindow.set_policy(PolicyType::Automatic, PolicyType::Automatic);

    contentpage.append(&scrollwindow);
    let loadmorebox = Button::builder()
        .child(&Image::from_icon_name("go-down-symbolic"))
        .hexpand(true)
        .vexpand(true)
        .halign(Align::Center)
        .valign(Align::End)
        .margin_bottom(15)
        .margin_top(0)
        .css_classes(vec!["pill", "1pill", "1flat", "1suggested-action"])
        .build();
    //contentpage.append(&scrollwindow);
    let flowcontentbox = GtkBox::new(Orientation::Vertical, 0);
    flowcontentbox.set_vexpand(true);
    flowcontentbox.set_hexpand(true);

    flowcontentbox.append(&flowbox);
    scrollwindow.set_child(Some(&flowcontentbox));
    flowcontentbox.append(&loadmorebox);

    let (sender, receiver) = async_channel::unbounded::<ProductCatalog>();
    let (loadmoresender, loadmorereceiver) = async_channel::unbounded::<ProductCatalog>();
    let productpage_ref = Arc::new(Mutex::new(productpage.clone()));
    let loadmore_productpage_ref = Arc::clone(&productpage_ref);
    loadmorebox.connect_clicked(move |loadmorebox| {
        loadmorebox.set_sensitive(false);
        loadmorebox.set_child(Some(
            &Spinner::builder()
                .height_request(24)
                .width_request(24)
                .build(),
        ));

        //println!("_contentbox widget has been realized");
        let sender = loadmoresender.clone();
        let loadmore_productpage_ref = loadmore_productpage_ref.clone();

        // Run async code to get all required values for populating full icon themes
        adw::gio::spawn_blocking(move || {
            let mut productpage_mutex = loadmore_productpage_ref.lock().unwrap();
            let productprops = productpage_mutex.deref_mut();
            productprops.set_page(productprops.pageno + 1);
            let productcatalog: ProductCatalog = get_product_catalog(&productprops).unwrap();
            //downloadthumbs(productcatalog.data.clone()).unwrap();
            sender.send_blocking(productcatalog).unwrap_or_default();
        });
    });
    let contentbox_productpage_ref = Arc::clone(&productpage_ref);
    themecategory_contentbox.connect_realize(move |_contentbox| {
        //println!("_contentbox widget has been realized");
        let sender = sender.clone();
        let productpage_ref = Arc::clone(&contentbox_productpage_ref);
        // Run async code to get all required values for populating themes
        adw::gio::spawn_blocking(move || {
            let productpage_mutex = productpage_ref.lock().unwrap();
            let productpage = productpage_mutex.deref();
            let productprops = productpage.clone();

            let productcatalog: ProductCatalog = get_product_catalog(&productprops).unwrap();
            //downloadthumbs(productcatalog.data.clone()).unwrap();
            sender.send_blocking(productcatalog).unwrap_or_default();
        });
    });

    // The main loop executes the asynchronous block
    let window: ApplicationWindow = window.clone();
    glib::spawn_future_local({
        async move {
            if let Ok(productcatalog) = receiver.recv().await {
                for each_product in productcatalog.data {
                    build_flowbox_for_page(&each_product, &flowbox, &window);
                }
                themecategory_loadingpage.remove(&themecategory_contentbox);
                themecategory_loadingpage.append(&contentpage);
            }

            while let Ok(productcatalog) = loadmorereceiver.recv().await {
                for each_product in productcatalog.data {
                    build_flowbox_for_page(&each_product, &flowbox, &window);
                    //loadmorebox.set_child(&None);
                    loadmorebox.set_child(Some(&Image::from_icon_name("go-down-symbolic")));
                    loadmorebox.set_sensitive(true);
                }
            }
        }
    });
}

fn build_search_content_box(
    searchentry: &SearchEntry,
    searchresultpage: &GtkBox,
    window: &ApplicationWindow,
) {
    let search_contentbox = GtkBox::new(Orientation::Vertical, 20);
    println!("Inside the seach content box");
    search_contentbox.set_widget_name("SearchContentBox");
    //window.set_height_request(1024);
    search_contentbox.set_valign(Align::Center);
    search_contentbox.set_halign(Align::Center);

    search_contentbox.set_vexpand(true);
    search_contentbox.set_hexpand(true);

    let themecategory_loadingpage = GtkBox::new(Orientation::Vertical, 0);
    themecategory_loadingpage.set_widget_name("themecategory_loadingpage");
    themecategory_loadingpage.append(&search_contentbox);
    //_page async loading of first page

    searchresultpage.append(&themecategory_loadingpage);

    let searchcontentpage = GtkBox::new(Orientation::Vertical, 0);
    //contentpage.set_css_classes(&vec!["card"]);

    let flowbox = FlowBox::builder().build();
    flowbox.set_vexpand(true);
    flowbox.set_hexpand(true);
    flowbox.set_valign(Align::Center);
    flowbox.set_halign(Align::Center);
    flowbox.set_activate_on_single_click(false);
    flowbox.set_min_children_per_line(1);
    flowbox.set_max_children_per_line(5);
    flowbox.set_selection_mode(SelectionMode::None);
    flowbox.set_css_classes(&vec!["suggested-action"]);
    let flowboxrevealer = adw::gtk::Revealer::new();
        flowboxrevealer.set_transition_type(adw::gtk::RevealerTransitionType::Crossfade);
        flowboxrevealer.set_transition_duration(3000); // in milliseconds
        flowboxrevealer.set_child(Some(&flowbox));
    flowboxrevealer.set_reveal_child(true);


    let scrollwindow = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    scrollwindow.set_policy(PolicyType::Automatic, PolicyType::Automatic);

    searchcontentpage.append(&scrollwindow);

    //searchcontentpage.append(&scrollwindow);
    let flowcontentbox = GtkBox::new(Orientation::Vertical, 0);
    flowcontentbox.set_vexpand(true);
    flowcontentbox.set_hexpand(true);

    flowcontentbox.append(&flowboxrevealer);
    scrollwindow.set_child(Some(&flowcontentbox));
    println!("Inside the search content box flowcontentbox");
    let (sender, receiver) = async_channel::unbounded::<(String, ProductCatalog)>();
    //let (loadmoresender, loadmorereceiver) = async_channel::unbounded::<ProductCatalog>();
    //let productpage_ref = Arc::new(Mutex::new(searchpageprops.clone()));
    //let loadmore_productpage_ref: Arc<Mutex<SearchPageProps>> = Arc::clone(&productpage_ref);
    let productpage = SearchPageProps::default(searchentry.text().to_string());

    //let contentbox_productpage_ref = Arc::clone(&productpage_ref);
    println!("Inside the seach content box2");
    let firstloadsender = sender.clone();
    let product_ref = Arc::new(Mutex::new(productpage));
    let product_loadmore_ref = Arc::clone(&product_ref);
    //let flowbox_clone: FlowBox = flowbox.clone();
    let flowboxrevealer_clone = flowboxrevealer.clone();
    searchentry.connect_search_changed(move |searchentry| {

        println!("search_contentbox widget has been changed");
        let sender = firstloadsender.clone();
        let sender_ref = Arc::new(Mutex::new(sender.clone()));
        let mut productpage_mutex = product_loadmore_ref.lock().unwrap();
        let productpage = productpage_mutex.deref_mut();
        productpage.set_search_text(searchentry.text().to_string());
        let productpage_ref = Arc::new(Mutex::new(productpage.clone()));
        //let productpage_ref = Arc::clone(&contentbox_productpage_ref);

        //let flowbox_ref = Arc::clone(&flowbox_ref);
        //let flowbox_ref = flowbox_ref.lock().unwrap();
        //let flowbox = flowbox_ref.deref();
        // Run async code to get all required values for populating themes
        adw::gio::spawn_blocking(move || {
            let sender_ref = sender_ref.lock().unwrap();
            let sender = sender_ref.deref();
            let productpage_mutex = productpage_ref.lock().unwrap();
            let productpage = productpage_mutex.deref();
            //let productprops = productpage;

            let productcatalog: ProductCatalog = get_search_product_catalog(&productpage).unwrap();
            //downloadthumbs(productcatalog.data.clone()).unwrap();
            sender
                .send_blocking(("firstload".to_string(), productcatalog))
                .unwrap_or_default();
        });
    });

    // The main loop executes the asynchronous block
    let window: ApplicationWindow = window.clone();
    glib::spawn_future_local({
        async move {
            while let Ok((message, productcatalog)) = receiver.recv().await {
                println!("Search Entry Changed Recv");
                if message.eq("firstload") {
                    flowboxrevealer_clone.set_reveal_child(false);
                    while flowbox.first_child().is_some() {
                        let child = flowbox.first_child().unwrap();
                        flowbox.remove(&child);
                    }
                    while themecategory_loadingpage.first_child().is_some() {
                        let child = themecategory_loadingpage.first_child().unwrap();
                        themecategory_loadingpage.remove(&child);
                    }
                    for each_product in productcatalog.data {
                        build_flowbox_for_page(&each_product, &flowbox, &window);
                    }
                    //themecategory_loadingpage.remove(&search_contentbox);

                    themecategory_loadingpage.append(&searchcontentpage);
                    flowboxrevealer.set_reveal_child(true);
                }
            }
        }
    });
}

fn main() -> glib::ExitCode {
    // Initialize GTK
    adw::init().unwrap();
    load_custom_css();

    // Create a new application
    let app = adw::Application::builder()
        .application_id("io.github.debasish_patra_1987.linuxthemestore")
        .build();

    app.connect_activate(build_ui);

    app.run()
}

fn build_ui(app: &adw::Application) {
    // Header bar and view switcher
    let header_bar = adw::HeaderBar::new();
    let header_box = GtkBox::new(Orientation::Vertical, 10);
    header_box.set_css_classes(&vec!["background"]);
    //header_bar.append();
    header_box.append(&header_bar);

    // Initial Screen Widgets below Starts
    // View Switcher
    let view_switcher = adw::InlineViewSwitcher::new();
    //view_switcher.add_css_class("round");

    // View Stack
    let view_stack = adw::ViewStack::new();
    view_stack.set_enable_transitions(true);
    view_stack.add_css_class("background");
    //view_stack.set_transition_duration(20);
    view_switcher.set_stack(Some(&view_stack));

    // Header Bar Setup below
    header_box.set_hexpand(true);
    header_box.set_vexpand(true);
    view_switcher.set_can_shrink(true);

    let view_switcher_box = GtkBox::new(Orientation::Horizontal, 0);
    view_switcher_box.set_halign(Align::Start);
    view_switcher_box.append(&view_switcher);
    header_bar.set_title_widget(Some(&view_switcher_box));

    // Add About in header bar ends

    let outer_view_stack = GtkBox::new(Orientation::Vertical, 0);
    outer_view_stack.append(&view_stack);
    header_box.append(&outer_view_stack);

    // Create main application window
    let window = ApplicationWindow::builder()
        .application(app)
        .content(&header_box)
        .default_width(1980)
        .default_height(1080)
        .build();

    let about_button = Button::from_icon_name("dialog-information-symbolic");
    header_bar.pack_end(&about_button);

    let window_clone = window.clone();
    about_button.connect_clicked(move |_| {
        let about_dialog = AboutDialog::builder()
            .application_name("Linux Theme Store")
            .developer_name("Debasish Patra")
            .application_icon("io.github.debasish_patra_1987.linuxthemestore")
            .version("1.0.2")
            .license_type(License::Gpl30)
            .comments("Download and Install Desktop Themes")
            .build();

        about_dialog.present(Some(&window_clone));
    });

    for each_catalog_type in Catalog::get_all_catalog_types() {
        build_category_page(&view_stack, &outer_view_stack, &each_catalog_type, &window);
    }
    build_search_page(&view_stack, &outer_view_stack, &window);
    window.present();
}

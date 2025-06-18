use adw::gio::prelude::ListModelExt;
use adw::gio::prelude::ListModelExtManual;
use adw::gio::prelude::SettingsExt;

use adw::gio::Settings;
use adw::glib::object::Cast;
use adw::glib::object::IsA;
use adw::glib::object::ObjectExt;
use adw::glib::types::StaticType;
use adw::gtk::DrawingArea;
use adw::gtk::SearchEntry;
use adw::gtk::StringObject;
use adw::prelude::ComboRowExt;
use adw::prelude::PreferencesPageExt;
use adw::prelude::{ActionRowExt, AdwDialogExt, ExpanderRowExt, PreferencesGroupExt};
use adw::Carousel;
use adw::CarouselIndicatorDots;
use adw::ComboRow;
use adw::PreferencesPage;
use chrono::DateTime;
use gtk4::prelude::AdjustmentExt;
use gtk4::prelude::{ButtonExt, DrawingAreaExt, DrawingAreaExtManual, EditableExt};
use gtk4::{Button, ContentFit, CssProvider, GestureClick, License};
use reqwest::blocking::Client;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use walkdir::WalkDir;

use std::ops::{Deref, DerefMut};
use std::path::Path;
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
            source
                .chars()
                .filter(|&c| match c {
                    '<' => {
                        inside = true;
                        false
                    }
                    '>' => {
                        inside = false;
                        false
                    }
                    _ => !inside,
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
            &SortType::Rating,
            &SortType::Latest,
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
    Wallpapers,
}
impl Catalog {
    pub fn get_id(&self) -> &str {
        match &self {
            Catalog::FullIconThemes => "132",
            Catalog::Cursors => "107",
            Catalog::GnomeShellThemes => "134",
            Catalog::Gtk4Themes => "135",
            Catalog::Wallpapers => "295",
        }
    }
    pub fn to_string(&self) -> &str {
        match &self {
            Catalog::FullIconThemes => "Icon",
            Catalog::Cursors => "Cursor",
            Catalog::GnomeShellThemes => "Gnome Shell",
            Catalog::Gtk4Themes => "Gtk",
            Catalog::Wallpapers => "Wallpapers",
        }
    }
    pub fn id_to_string(&self) -> &str {
        match self.get_id() {
            "132" => "Icon",
            "107" => "Cursor",
            "134" => "Gnome Shell",
            "135" => "Gtk",
            "295" => "Wallpapers",
            _ => "Unknown",
        }
    }
    pub fn id_to_catalog(id: &str) -> Catalog {
        match id {
            "132" => Catalog::FullIconThemes,
            "107" => Catalog::Cursors,
            "134" => Catalog::GnomeShellThemes,
            "135" => Catalog::Gtk4Themes,
            "295" => Catalog::Wallpapers,
            _ => Catalog::Wallpapers,
        }
    }
    pub fn get_all_catalog_types() -> Vec<Catalog> {
        vec![
            Catalog::FullIconThemes,
            Catalog::Cursors,
            Catalog::GnomeShellThemes,
            Catalog::Gtk4Themes,
            Catalog::Wallpapers,
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
            + "/ocs/v1/content/data?format=json&categories=132,107,134,135,295&pagesize="
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
        true => {}
        false => {
            let _res = fetch_url(&downloaddetail.downloadlink, path.clone());
        }
    }
    let _ = install_tar(&path.clone(), &themetype).unwrap();
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
        Catalog::Wallpapers => {
            extract_path.push(".local/share/wallpapers");
        }
    }
    println!("Installation Path : {}", &extract_path.display());
    fs::create_dir_all(&extract_path)?;
    println!("Theme Type : {:?}", theme_type);
    println!("Path : {}", &path);
    println!("extract_path : {}", &extract_path.display());

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
    } else if path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".png")
        || path.ends_with(".svg")
    {
        Command::new("cp")
            .arg(&path)
            .arg(&extract_path)
            .output()
            .expect("Failed to move the wallpaper");
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
                        border-width: 1px;
                        border-color: white;
                    }
                    ",
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
    //println!(        "{}",        serde_json::to_string_pretty(&resp_json_products).unwrap()    );
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

        fetch_url(&firstimage.replace("770x540", "512x512"), save_path).unwrap();
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

            fetch_url(&each_image.replace("770x540", "512x512"), save_path).unwrap();
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

                    fetch_url(&image_small.replace("770x540", "512x512"), save_path).unwrap();
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
    themecategorysortbybutton.add_css_class("round");
    themecategorysortbybutton.set_can_shrink(true);

    let themecategorysortby_view_stack = adw::ViewStack::builder().enable_transitions(true).build();

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
        view_stack.add_titled(&searchbox, Some("Search"), "Search");

    let searchinput = SearchEntry::new();
    searchinput.set_margin_top(10);
    searchinput.set_search_delay(0);
    searchinput.set_placeholder_text(Some("Search Themes and Wallpapers"));

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
    //println!("{}", searchinput.text().to_string());

    //    let searchpageprops = SearchPageProps::default(searchinput.text().to_string() );
}

fn build_installed_page(
    view_stack: &ViewStack,
    outer_view_stack: &GtkBox,
    theme_type: &Catalog,
    window: &ApplicationWindow,
) {
    let themecategoryloadingpage = GtkBox::new(Orientation::Vertical, 10);
    themecategoryloadingpage.add_css_class("background");
    let _themecategorypage_viewstack = view_stack.add_titled(
        &themecategoryloadingpage,
        Some(&theme_type.to_string()),
        theme_type.to_string(),
    );

    let themecategorysortbybutton = adw::InlineViewSwitcher::new();
    themecategorysortbybutton.add_css_class("round");
    themecategorysortbybutton.set_can_shrink(true);

    let themecategorysortby_view_stack = adw::ViewStack::builder().enable_transitions(true).build();

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

    /*
    build_content_box(
        ProductPageProps::default()
            .set_catalog(theme_type.to_owned())
            .set_order(each_sorting_type.to_owned()),
        &themecategorysortby_view_stack,
        &window,
    );
    */
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
        //.css_name("img-cover")
        .build();
    img.add_css_class("img-round");
    img.set_content_fit(ContentFit::Cover);
    //img.set_filename(Some(&std::path::Path::new(imgpath.as_str())));
    img.set_size_request(260, 260);
    img.set_can_shrink(true);
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
        .css_name("img-cover")
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
            //.label(Catalog::id_to_string(&each_product.typeid.to_string()))
            .label(&each_product.typename.to_string())
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
            //downloadotherimages(&each_prod_clone).unwrap_or_default();
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
        let (sender, receiver) = async_channel::unbounded::<String>();
        let product_clone = product.clone();
        adw::gio::spawn_blocking(move || {
            downloadotherimages(&product_clone).unwrap_or_default();
            sender
                .send_blocking("allimagesdownloaded".to_string())
                .unwrap_or_default();
        });


        let imgclamp = Clamp::new();
        imgclamp.set_css_classes(&vec!["clamp"]);
        imgclamp.set_tightening_threshold(256);
        imgclamp.set_maximum_size(256);
        imgclamp.set_size_request(512, 512);
        imgclamp.set_margin_top(20);
        imgclamp.set_margin_bottom(20);

        //imgclamp.hide();
        let allimagespinner = Spinner::builder()
            .valign(Align::Center)
            .halign(Align::Center)
            .hexpand(true)
            .vexpand(true)
            .width_request(32)
            .height_request(32)
            .build();
        //allimagespinner.show();
        imgclamp.set_child(Some(&allimagespinner));

        let imgclamp_clone = imgclamp.clone();
        let previewpics_clone = product.previewpics.clone();
        glib::spawn_future_local({
            async move {
                while let Ok(message) = receiver.recv().await {
                    if message.eq(&String::from("allimagesdownloaded")) {
                        //allimagespinner.hide();

                        let carousal = Carousel::new();
                        let carousal_ind = CarouselIndicatorDots::builder().carousel(&carousal).build();

                        for path in &previewpics_clone{
                        let imgpath = "/tmp/themeinstaller/cache/".to_string() + path;
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
                        //img.set_size_request(512, 512);
                        img.set_width_request(512);
                        img.set_content_fit(ContentFit::Fill);
                        img.set_filename(Some(&std::path::Path::new(imgpath.as_str())));

                        let imgbox = GtkBox::new(Orientation::Vertical,5);
                        imgbox.append(&img);
                        //imgbox.append(&Label::new(Some(path)));
                        carousal.append(&imgbox);
                        }
                        //Insert Images in dialog body

                        let each_img_box = GtkBox::builder()
                            .spacing(10)
                            .orientation(Orientation::Horizontal)
                            .vexpand(false)
                            .hexpand(false)
                            .build();

                        let  carouselbox = GtkBox::new(Orientation::Vertical,0);
                        carouselbox.append(&carousal);
                        carouselbox.append(&carousal_ind);
                        each_img_box.append(&carouselbox);


                        imgclamp_clone.set_child(Some(&each_img_box));


                    } else {
                    }
                }
            }
        });

        dialogbody.append(&imgclamp);

        dialog.set_child(Some(&dialogbox));
        let producttype = &product.typename.clone().to_string();
        let preftitle = match producttype.as_str(){
            "Full Icon Themes" => "~/.local/share/icons",
            "Cursors" => "~/.local/share/icons",
            "Gnome Shell Themes" => "~/.local/share/themes",
            "GTK3/4 Themes" => "~/.local/share/themes",
            _ => "~/.local/share/wallpapers",
        };
        let group = PreferencesGroup::builder()
            .title(format!("Installed in {}",preftitle))
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
                //.css_classes(vec!["flat"])
                .icon_name("folder-download-symbolic")
                .tooltip_text("Download Theme")
                .margin_bottom(10)
                .margin_top(10)
                .sensitive(true)
                .build();
            row.add_suffix(&downloadbutton);
            let new_variant = each_variant.clone();
            let catalogtype = Catalog::id_to_catalog(&product.typeid.to_string().as_str());
            //println!("Catalog Name : {:?}",&product);
            //println!("Catalog Typeid : {:?}",&product.typeid.to_string().as_str());
            //println!("Catalog Type : {:?}",catalogtype.get_id());

            let (senderdownload, receiverdownload) = async_channel::unbounded::<String>();
            downloadbutton.connect_clicked(move |downloadbutton| {
                downloadbutton.set_child(Some(&Spinner::new()));
                //eprintln!("Clicked!");

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
                                downloadbutton_clone.set_icon_name("test-pass-symbolic");
                                downloadbutton_clone.set_tooltip_text(Some("Apply"));
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
    /*
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
     */
    //contentpage.append(&scrollwindow);
    let flowcontentbox = GtkBox::new(Orientation::Vertical, 0);
    flowcontentbox.set_vexpand(true);
    flowcontentbox.set_hexpand(true);

    flowcontentbox.append(&flowbox);
    scrollwindow.set_child(Some(&flowcontentbox));
    let flowboxloading = Spinner::new();
    flowboxloading.set_width_request(32);
    flowboxloading.set_height_request(32);
    flowcontentbox.append(&flowboxloading);
    flowboxloading.hide();
    //flowcontentbox.append(&loadmorebox);

    let (sender, receiver) = async_channel::unbounded::<ProductCatalog>();
    let (loadmoresender, loadmorereceiver) = async_channel::unbounded::<ProductCatalog>();
    let productpage_ref = Arc::new(Mutex::new(productpage.clone()));
    let loadmore_productpage_ref = Arc::clone(&productpage_ref);
    /*loadmorebox.connect_clicked(move |loadmorebox| {
        loadmorebox.set_sensitive(false);
        loadmorebox.set_child(Some(
            &Spinner::builder()
                .height_request(24)
                .width_request(24)
                .build(),
        ));

        //println!("_contentbox widget has been realized");

    }); */

    let vadjustment = scrollwindow.vadjustment();
    let vadjustment_clone = vadjustment.clone();
    let flowboxloading_clone = flowboxloading.clone();
    let loadmoresender_clone = loadmoresender.clone();
    let loadmore_productpage_ref_clone = loadmore_productpage_ref.clone();
    vadjustment.connect_value_changed(move |vadjustment| {
        let value = vadjustment.value();
        let upper = vadjustment.upper();
        let page_size = vadjustment.page_size();

        if (value + page_size) >= (upper - 0.5) {
            flowboxloading_clone.show();
            let sender = loadmoresender_clone.clone();
            let loadmore_productpage_ref = loadmore_productpage_ref_clone.clone();

            // Run async code to get all required values for populating full icon themes
            adw::gio::spawn_blocking(move || {
                let mut productpage_mutex = loadmore_productpage_ref.lock().unwrap();
                let productprops = productpage_mutex.deref_mut();
                productprops.set_page(productprops.pageno + 1);
                let productcatalog: ProductCatalog = get_product_catalog(&productprops).unwrap();
                //downloadthumbs(productcatalog.data.clone()).unwrap();
                sender.send_blocking(productcatalog).unwrap_or_default();
            });
        }
    });
    let flowboxloading_clone = flowboxloading.clone();
    let loadmoresender_clone = loadmoresender.clone();
    let loadmore_productpage_ref_clone = loadmore_productpage_ref.clone();
    scrollwindow.connect_map(move |_| {
        let value = vadjustment_clone.value();
        let upper = vadjustment_clone.upper();
        let page_size = vadjustment_clone.page_size();

        if (value + page_size) >= (upper - 0.5) {
            flowboxloading_clone.show();
            let sender = loadmoresender_clone.clone();
            let loadmore_productpage_ref = loadmore_productpage_ref_clone.clone();

            // Run async code to get all required values for populating full icon themes
            adw::gio::spawn_blocking(move || {
                let mut productpage_mutex = loadmore_productpage_ref.lock().unwrap();
                let productprops = productpage_mutex.deref_mut();
                productprops.set_page(productprops.pageno + 1);
                let productcatalog: ProductCatalog = get_product_catalog(&productprops).unwrap();
                //downloadthumbs(productcatalog.data.clone()).unwrap();
                sender.send_blocking(productcatalog).unwrap_or_default();
            });
        }
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
    let flowboxloading_clone = flowboxloading.clone();
    glib::spawn_future_local({
        async move {
            if let Ok(productcatalog) = receiver.recv().await {
                for each_product in productcatalog.data {
                    build_flowbox_for_page(&each_product, &flowbox, &window);
                }
                themecategory_loadingpage.remove(&themecategory_contentbox);
                themecategory_loadingpage.append(&contentpage);
                flowboxloading_clone.hide();
            }

            while let Ok(productcatalog) = loadmorereceiver.recv().await {
                for each_product in productcatalog.data {
                    build_flowbox_for_page(&each_product, &flowbox, &window);
                    //loadmorebox.set_child(&None);
                    //loadmorebox.set_child(Some(&Image::from_icon_name("go-down-symbolic")));
                    //loadmorebox.set_sensitive(true);
                    flowboxloading_clone.hide();
                }
            }
        }
    });
}

fn build_installed_content_box(
    installed_page: &InstalledTheme,
    view_stack: &ViewStack,
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
        .label(String::from("Fetching ") + installed_page.name.to_string() + ". Please wait...")
        .css_classes(vec!["dimmed", "Heading-4"])
        .build();

    themecategory_contentbox.append(&spinner_loading_themecategory_latest);
    themecategory_contentbox.append(&spinner_label_themecategory_latest);
    let themecategory_loadingpage = GtkBox::new(Orientation::Vertical, 0);
    themecategory_loadingpage.append(&themecategory_contentbox);
    let _fulliconpage_viewstack_latest = view_stack.add_titled(
        &themecategory_loadingpage,
        Some(installed_page.name.to_string()),
        installed_page.name.to_string(),
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
    /*
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
     */
    //contentpage.append(&scrollwindow);
    let flowcontentbox = GtkBox::new(Orientation::Vertical, 0);
    flowcontentbox.set_vexpand(true);
    flowcontentbox.set_hexpand(true);

    flowcontentbox.append(&flowbox);
    scrollwindow.set_child(Some(&flowcontentbox));

    let installed_box = GtkBox::builder().build();
    installed_box.append(&Label::new(Some("X")));

    flowcontentbox.append(&installed_box);
    //    flowboxloading.hide();
    //flowcontentbox.append(&loadmorebox);
    // The main loop executes the asynchronous block
}

fn build_search_content_box(
    searchentry: &SearchEntry,
    searchresultpage: &GtkBox,
    window: &ApplicationWindow,
) {
    let search_contentbox = GtkBox::new(Orientation::Vertical, 20);
    //println!("Inside the seach content box");
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
    //println!("Inside the search content box flowcontentbox");
    let (sender, receiver) = async_channel::unbounded::<(String, ProductCatalog)>();
    //let (loadmoresender, loadmorereceiver) = async_channel::unbounded::<ProductCatalog>();
    //let productpage_ref = Arc::new(Mutex::new(searchpageprops.clone()));
    //let loadmore_productpage_ref: Arc<Mutex<SearchPageProps>> = Arc::clone(&productpage_ref);
    let productpage = SearchPageProps::default(searchentry.text().to_string());

    //let contentbox_productpage_ref = Arc::clone(&productpage_ref);
    //println!("Inside the seach content box2");
    let firstloadsender = sender.clone();
    let product_ref = Arc::new(Mutex::new(productpage));
    let product_loadmore_ref = Arc::clone(&product_ref);
    //let flowbox_clone: FlowBox = flowbox.clone();
    let flowboxrevealer_clone = flowboxrevealer.clone();

    searchentry.connect_search_changed(move |searchentry| {
        //println!("search_contentbox widget has been changed");
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
    //header_box.set_css_classes(&vec!["background"]);
    //header_bar.append();

    // Initial Screen Widgets below Starts
    // View Switcher
    let view_switcher = adw::InlineViewSwitcher::new();
    view_switcher.add_css_class("round");

    // View Stack
    let view_stack = adw::ViewStack::builder().enable_transitions(true).build();
    view_stack.add_css_class("background");
    view_switcher.set_stack(Some(&view_stack));

    // Header Bar Setup below
    header_box.set_hexpand(true);
    header_box.set_vexpand(true);
    header_box.set_margin_start(0);
    header_box.set_margin_end(0);
    header_box.set_margin_top(10);
    header_box.set_margin_bottom(10);
    view_switcher.set_can_shrink(true);

    let view_switcher_box = GtkBox::new(Orientation::Horizontal, 0);
    view_switcher_box.set_halign(Align::Center);
    view_switcher_box.append(&view_switcher);

    // Add About in header bar ends

    let outer_view_stack = GtkBox::new(Orientation::Vertical, 0);
    outer_view_stack.append(&view_stack);

    let bodybox = GtkBox::new(Orientation::Vertical, 0);
    // Create main application window
    let window = ApplicationWindow::builder()
        .application(app)
        .content(&bodybox)
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
            .version("1.0.4")
            .license_type(License::Gpl30)
            .comments("Download and Install Desktop Themes")
            .build();

        about_dialog.present(Some(&window_clone));
    });

    let body_switcher_box = adw::InlineViewSwitcher::new();
    body_switcher_box.add_css_class("round");
    body_switcher_box.set_valign(Align::Start);
    body_switcher_box.set_halign(Align::Center);

    let body_viewstack = adw::ViewStack::builder().enable_transitions(true).build();
    let _body_viewstack = body_viewstack.add_titled(&header_box, Some("Browse"), "Browse");

    bodybox.append(&header_bar);
    bodybox.append(&header_box);
    bodybox.append(&body_viewstack);

    header_box.append(&view_switcher_box);
    header_box.append(&outer_view_stack);
    header_bar.set_title_widget(Some(&body_switcher_box));
    body_switcher_box.set_stack(Some(&body_viewstack));

    for each_catalog_type in Catalog::get_all_catalog_types() {
        build_category_page(&view_stack, &outer_view_stack, &each_catalog_type, &window);
    }
    build_search_page(&body_viewstack, &outer_view_stack, &window);

    let installed_themes_box = GtkBox::new(Orientation::Vertical, 5);
    installed_themes_box.set_widget_name("Installed_themes_box");
    installed_themes_box.set_width_request(500);

    //let themes = get_all_installed_themes();

    let _body_viewstack =
        body_viewstack.add_titled(&installed_themes_box, Some("Installed"), "Installed");


    /*
    installed_themes_box.append(&fulliconprefpage);
    installed_themes_box.append(&cursorprefpage);
    installed_themes_box.append(&gnomeshellprefpage);
    installed_themes_box.append(&gtk4prefpage);
    */

    //let installed_themes_Switcher = InlineViewSwitcher::builder().build();
    //populate_installed_themes_page(themes,&installed_themes_box, page.clone());

    body_viewstack.connect_visible_child_name_notify(move |view_stack| {
    //installed_themes_box.remove(&page);

        println!("connect_notify_local triggered : {}",view_stack.visible_child().unwrap().widget_name());
        if let Some(visible_child) = view_stack.visible_child() {
            if visible_child.widget_name() == "Installed_themes_box"{
                if installed_themes_box.last_child().is_some(){
                    let child = installed_themes_box.last_child().unwrap();
                    installed_themes_box.remove(&child);
                }
            println!("connect_notify_local triggered Install page");


            let page = PreferencesPage::new();
            page.set_vexpand(true);
            page.set_valign(Align::Center);
            installed_themes_box.append(&page);
            populate_installed_themes_page(get_all_installed_themes(),page);
            }
        }
    });

    window.present();
}

pub fn populate_installed_themes_page(themes: Vec<InstalledTheme>, page: PreferencesPage) {


    for each_item in themes {
        // Create the ListStore model
        let model: adw::gio::ListStore =
            adw::gio::ListStore::with_type(StringObject::static_type());
        model.append(&StringObject::new(""));
        let model_clone = model.clone();

        match each_item.name {
            Catalog::FullIconThemes => {

                // Populate the model dynamically
                for item in &each_item.options {
                    let itemstr = StringObject::new(&item.clone());
                    if !item.eq("Adwaita"){
                        model.append(&itemstr);
                    }
                }

                // Create the ComboRow
                let combo = ComboRow::builder()
                    .title(each_item.name.to_string())
                    .model(&model)
                    .build();

                // Handle selection changes
                combo.connect_selected_notify(move |row| {
                    if let Some(item) = model.item(row.selected()) {
                        let value = item.downcast_ref::<StringObject>().unwrap();
                        println!("Selected: {}", value.string());
                        if value.string().eq(""){
                            apply_theme(each_item.name.clone(), &value.string());
                        }
                    }
                });

                let name = Catalog::FullIconThemes;
                /*
                let index = each_item
                    .options
                    .iter()
                    .position(|s| s.eq_ignore_ascii_case(&get_applied_theme(name.clone())))
                    .unwrap_or_else(||0);
                 */
                let options = each_item.options;

                let index = match options
                    .iter()
                    .position(|s| s.eq_ignore_ascii_case(&get_applied_theme(name.clone()))){
                        Some(index) => index + 1,
                        _ =>  0,
                    };
                println!("Icon THeme Index : {}",index);
                println!("Icon THeme Options : {:#?}",options);
                let index = u32::try_from(index).expect("Value too large for u32");
                combo.set_selected(index);

                // Pack into preferences UI
                let group = PreferencesGroup::new();
                group.add(&combo);

                page.add(&group);
            }
            Catalog::Cursors => {

                // Populate the model dynamically
                for item in &each_item.options {
                    let itemstr = StringObject::new(&item.clone());
                    if !item.eq("Adwaita"){
                        model.append(&itemstr);
                    }
                }

                // Create the ComboRow
                let combo = ComboRow::builder()
                    .title(each_item.name.to_string())
                    .model(&model)
                    .build();

                // Handle selection changes
                combo.connect_selected_notify(move |row| {
                    if let Some(item) = model.item(row.selected()) {
                        let value = item.downcast_ref::<StringObject>().unwrap();
                        println!("Selected: {}", value.string());
                        if value.string().eq(""){
                            apply_theme(each_item.name.clone(), &value.string());
                        }
                    }
                });

                let name = Catalog::Cursors;
                let options = each_item.options;

                let index = match options
                    .iter()
                    .position(|s| s.eq_ignore_ascii_case(&get_applied_theme(name.clone()))){
                        Some(index) => index + 1,
                        _ => 0,
                    };
                let index = u32::try_from(index).expect("Value too large for u32");
                combo.set_selected(index);
                // Pack into preferences UI
                let group = PreferencesGroup::new();
                group.add(&combo);

                group.set_title("Change Styles");
                page.add(&group);
            }/*
            Catalog::GnomeShellThemes => {
                // Create the ListStore model
                let model: adw::gio::ListStore =
                    adw::gio::ListStore::with_type(StringObject::static_type());

                // Populate the model dynamically
                for item in &each_item.options {
                    let itemstr = StringObject::new(&item.clone());
                    model.append(&itemstr);
                }

                // Create the ComboRow
                let combo = ComboRow::builder()
                    .title(each_item.name.to_string())
                    .model(&model)
                    .build();

                // Handle selection changes
                combo.connect_selected_notify(move |row| {
                    if let Some(item) = model.item(row.selected()) {
                        let value = item.downcast_ref::<StringObject>().unwrap();
                        println!("Selected: {}", value.string());
                        apply_theme(each_item.name.clone(), &value.string());
                    }
                });
                let name = Catalog::GnomeShellThemes;
                let index = each_item
                    .options
                    .iter()
                    .position(|s| s.eq_ignore_ascii_case(&get_applied_theme(name.clone())))
                    .unwrap_or_else(||1);
                let index = u32::try_from(index).expect("Value too large for u32");
                combo.set_selected(index);
                // Pack into preferences UI
                let group = PreferencesGroup::new();
                group.add(&combo);

                page.add(&group);
            } */
            Catalog::Gtk4Themes => {

                // Populate the model dynamically
                for item in &each_item.options {
                    let itemstr = StringObject::new(&item.clone());
                    if !item.eq("Adwaita"){
                    model.append(&itemstr);
                    }
                }
                println!("Gtk THeme Options : {:#?}",model.clone());
                // Create the ComboRow
                let combo = ComboRow::builder()
                    .title(each_item.name.to_string())
                    .model(&model)
                    .build();

                // Handle selection changes
                combo.connect_selected_notify(move |row| {
                    if let Some(item) = model.item(row.selected()) {
                        let value = item.downcast_ref::<StringObject>().unwrap();
                        println!("Selected: {}", value.string());
                         if value.string().eq(""){
                            apply_theme(each_item.name.clone(), &value.string());
                        }
                    }
                });
                let name = Catalog::Gtk4Themes;
                println!("Catalog:: Gtk4Themes ::> {}",name.to_string());

                println!("Catalog:: Gtk4Themes ::> {}",get_applied_theme(name.clone()));
                let options = each_item.options;

                let index = match options
                    .iter()
                    .position(|s| s.eq_ignore_ascii_case(&get_applied_theme(name.clone()))){
                        Some(index) => index+1,
                        _ =>  0,
                    };
                println!("Gtk THeme Index : {}",index);

                let index = u32::try_from(index).expect("Value too large for u32");
                combo.set_selected(index);
                // Pack into preferences UI
                let group = PreferencesGroup::new();
                group.add(&combo);

                page.add(&group);
            }
            _ => {}
        };
    }
}

#[derive(Debug,Clone)]
pub struct InstalledTheme {
    name: Catalog,
    options: Vec<String>,
}
pub fn get_applied_theme(catalog: Catalog) -> String {
    match catalog {
        Catalog::Cursors => {
            let settings = Settings::new("org.gnome.desktop.interface");
            println!("Cursor Theme : {}", settings.string("cursor-theme").to_string());
            settings.string("cursor-theme").to_string()
        }
        Catalog::FullIconThemes => {
            let settings = Settings::new("org.gnome.desktop.interface");
            println!("Icon Theme : {}", settings.string("icon-theme").to_string());
            settings.string("icon-theme").to_string()
        }
        Catalog::GnomeShellThemes => {
            /*
            let settings = Settings::new("org.gnome.shell");
            settings
                .set_boolean("disable-user-extensions", false)
                .expect("Failed to set Gnome Shell theme");

            let settings = Settings::new("org.gnome.shell.extensions.user-theme");
            settings.string("name").to_string()
            */
            String::new()
        }
        Catalog::Gtk4Themes => {
            let settings = Settings::new("org.gnome.desktop.interface");
            println!("Gtk Theme : {}", settings.string("gtk-theme").to_string());
            settings.string("gtk-theme").to_string()
        }
        _ => String::from("Wallpapers are not supported"),
    }
}
pub fn apply_theme(catalog: Catalog, theme_name: &str) -> Result<()> {
    match catalog {
        Catalog::Cursors => {
            let settings = Settings::new("org.gnome.desktop.interface");
            settings
                .set_string("cursor-theme", theme_name)
                .expect("Failed to set Cursor theme");
        }
        Catalog::FullIconThemes => {
            let settings = Settings::new("org.gnome.desktop.interface");
            settings
                .set_string("icon-theme", theme_name)
                .expect("Failed to set Icon theme");
        }
        Catalog::GnomeShellThemes => {
            //let settings = Settings::new("org.gnome.shell.extensions.user-theme");
            //settings
            //    .set_string("name", theme_name)
            //    .expect("Failed to set Gnome Shell theme");

        }
        Catalog::Gtk4Themes => {
            let settings = Settings::new("org.gnome.desktop.interface");
            settings
                .set_string("gtk-theme", theme_name)
                .expect("Failed to set GTK theme");
        }
        _ => {}
    }
    Ok(())
}
pub fn get_all_installed_themes() -> Vec<InstalledTheme> {
    let home_dir = std::env::var("HOME").unwrap();
    println!("Home Dir : {}", home_dir);
    let cursor_icon_paths = [
        &(home_dir.clone() + "/.icons"),
        &(home_dir.clone() + "/.local/share/icons"),
        "/usr/share/icons",
    ];

    let target_filename = "index.theme";
    let search_string = "[Icon Theme]";
    //let root_path = "."; // change to your root dir
    let mut cursorthemes: Vec<String> = vec![];
    let mut iconthemes: Vec<String> = vec![];
    for cursor_icon_path in cursor_icon_paths {
        for entry in WalkDir::new(cursor_icon_path)
            .max_depth(2)
            .into_iter()
            .filter_map(|s| s.ok())
            .filter(|e| e.file_name() == target_filename && e.file_type().is_file())
        {
            //println!("Walkdir : {}", entry.path().display());
            let path = entry.path();
            match fs::read_to_string(path) {
                Ok(content) => {
                    //println!("content : {:?}", &content);
                    let lines = content.split('\n');

                    for line in lines {
                        if line.contains(search_string) {
                            let cursorpath =
                                path.display().to_string().replace("/index.theme", "") + "/cursors";

                            let cursor_path = Path::new(&cursorpath);

                            if Path::new(cursor_path).exists() {
                                //println!("{} : {}", path.display().to_string().replace(&target_filename, "").replace(&cursor_icon_path,"").replace("/",""),cursor_icon_path);
                                //println!("Cursor Theme : {}", &line);
                                //println!("cursorpath : {:?}", &cursorpath);
                                //(Catalog::Cursors, path.display().to_string().replace(&target_filename, "").replace(&cursor_icon_path,"").replace("/",""));
                                cursorthemes.push(
                                    path.display()
                                        .to_string()
                                        .replace(&target_filename, "")
                                        .replace(&cursor_icon_path, "")
                                        .replace("/", ""),
                                );
                            }

                            let iconpath =
                                path.display().to_string().replace("/index.theme", "");

                            let icon_path = Path::new(&iconpath);

                            if Path::new(icon_path).exists() {
                                //println!("{} : {}", path.display().to_string().replace(&target_filename, "").replace(&cursor_icon_path,"").replace("/",""),cursor_icon_path);
                                //println!("Cursor Theme : {}", &line);
                                //println!("cursorpath : {:?}", &cursorpath);
                                //(Catalog::Cursors, path.display().to_string().replace(&target_filename, "").replace(&cursor_icon_path,"").replace("/",""));
                                iconthemes.push(
                                    path.display()
                                        .to_string()
                                        .replace(&target_filename, "")
                                        .replace(&cursor_icon_path, "")
                                        .replace("/", ""),
                                );
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Could not read {}: {}", path.display(), e),
            }
        }
    }

    let gnome_shell_gtk4_paths = [
        &(home_dir.clone() + "/.themes"),
        &(home_dir.clone() + "/.local/share/themes"),
        "/usr/share/themes",
    ];

    let target_filename = "index.theme";
    let search_string = "Type=X-GNOME-Metatheme";
    //let root_path = "."; // change to your root dir
    let mut gnomeshellthemes: Vec<String> = vec![];
    let mut gtk4themes: Vec<String> = vec![];
    for gnome_shell_gtk4_path in gnome_shell_gtk4_paths {
        for entry in WalkDir::new(gnome_shell_gtk4_path)
            .max_depth(2)
            .into_iter()
            .filter_map(|s| s.ok())
            .filter(|e| e.file_name() == target_filename && e.file_type().is_file())
        {
            //println!("Walkdir : {}", entry.path().display());
            let path = entry.path();
            match fs::read_to_string(path) {
                Ok(content) => {
                    //println!("content : {:?}", &content);
                    let lines = content.split('\n');

                    for line in lines {
                        if line.contains(search_string) {
                            let gnomeshellpath =
                                path.display().to_string().replace("/index.theme", "")
                                    + "/gnome-shell";

                            let gnomeshell_path = Path::new(&gnomeshellpath);

                            if Path::new(gnomeshell_path).exists() {
                                //println!("{} : {}", path.display().to_string().replace(&target_filename, "").replace(&cursor_icon_path,"").replace("/",""),cursor_icon_path);
                                //println!("Cursor Theme : {}", &line);
                                //println!("cursorpath : {:?}", &cursorpath);
                                //(Catalog::Cursors, path.display().to_string().replace(&target_filename, "").replace(&cursor_icon_path,"").replace("/",""));
                                gnomeshellthemes.push(
                                    path.display()
                                        .to_string()
                                        .replace(&target_filename, "")
                                        .replace(&gnome_shell_gtk4_path, "")
                                        .replace("/", ""),
                                );
                            }

                            let gtk4path =
                                path.display().to_string().replace("/index.theme", "") + "/gtk-4.0";

                            let gtk4_path = Path::new(&gtk4path);

                            if Path::new(gtk4_path).exists() {
                                //println!("{} : {}", path.display().to_string().replace(&target_filename, "").replace(&cursor_icon_path,"").replace("/",""),cursor_icon_path);
                                //println!("Cursor Theme : {}", &line);
                                //println!("cursorpath : {:?}", &cursorpath);
                                //(Catalog::Cursors, path.display().to_string().replace(&target_filename, "").replace(&cursor_icon_path,"").replace("/",""));
                                gtk4themes.push(
                                    path.display()
                                        .to_string()
                                        .replace(&target_filename, "")
                                        .replace(&gnome_shell_gtk4_path, "")
                                        .replace("/", ""),
                                );
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Could not read {}: {}", path.display(), e),
            }
        }
    }

    let installed_themes: Vec<InstalledTheme> = vec![
        InstalledTheme {
            name: Catalog::Cursors,
            options: cursorthemes.clone(),
        },
        InstalledTheme {
            name: Catalog::FullIconThemes,
            options: iconthemes.clone(),
        },
        /*
        InstalledTheme {
            name: Catalog::GnomeShellThemes,
            options: gnomeshellthemes.clone(),
        },
         */
        InstalledTheme {
            name: Catalog::Gtk4Themes,
            options: gtk4themes.clone(),
        },
    ];
    //println!("Themes : \n {:#?}", installed_themes);
    //println!("Get Current Gtk Themes : \n {:#?}", get_applied_theme(Catalog::Gtk4Themes));
    //println!("Set Gtk Themes to adw-gtk3 : \n {:#?}", apply_theme(Catalog::Gtk4Themes,"adw-gtk3"));

    installed_themes
}

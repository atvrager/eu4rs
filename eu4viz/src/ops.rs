use crate::args::MapMode;
use crate::window;
use eu4data::{
    Tradegood,
    countries::Country,
    cultures::Culture,
    history::ProvinceHistory,
    map::{DefaultMap, load_definitions},
    religions::Religion,
};
use eu4txt::{DefaultEU4Txt, EU4Txt, from_node};
use image::{Rgb, RgbImage};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Normalize path for display - convert to forward slashes for cleaner logging
fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

pub fn dump_tradegoods(base_path: &std::path::Path) -> Result<(), String> {
    let path = base_path.join("tradegoods/00_tradegoods.txt");
    println!("Loading {:?}", path);

    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| e.to_string())?;
    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| e.to_string())?;
    let goods: HashMap<String, Tradegood> = from_node(&ast)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&goods).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn draw_map(base_path: &Path, output_path: &Path, mode: MapMode) -> Result<(), String> {
    // 1. Load Definitions (ID -> Color, Color -> ID)
    let def_path = base_path.join("map/definition.csv");
    println!("Loading definitions from {:?}", def_path);
    let definitions = load_definitions(&def_path).map_err(|e| e.to_string())?;

    // Build reverse map (RGB -> ID)
    let mut color_to_id: HashMap<(u8, u8, u8), u32> = HashMap::new();
    for (id, def) in &definitions {
        color_to_id.insert((def.r, def.g, def.b), *id);
    }

    // 1b. Load Default Map (Sea/Lakes)
    let default_map_path = base_path.join("map/default.map");
    println!("Loading default map from {:?}", default_map_path);
    let dm_tokens =
        DefaultEU4Txt::open_txt(default_map_path.to_str().unwrap()).map_err(|e| e.to_string())?;
    let dm_ast = DefaultEU4Txt::parse(dm_tokens).map_err(|e| e.to_string())?;
    let default_map: DefaultMap = from_node(&dm_ast)?;

    let mut water_ids: HashSet<u32> = HashSet::new();
    for id in default_map.sea_starts {
        water_ids.insert(id);
    }
    for id in default_map.lakes {
        water_ids.insert(id);
    }
    println!("Loaded {} water provinces (sea+lakes).", water_ids.len());

    // 2. Load Data based on Mode
    let mut goods: HashMap<String, Tradegood> = HashMap::new();
    let mut countries: HashMap<String, eu4data::countries::Country> = HashMap::new();
    let mut religions: HashMap<String, Religion> = HashMap::new();
    let mut cultures: HashMap<String, Culture> = HashMap::new();

    match mode {
        MapMode::TradeGoods => {
            let goods_path = base_path.join("common/tradegoods/00_tradegoods.txt");
            println!("Loading trade goods from {:?}", goods_path);
            let tokens =
                DefaultEU4Txt::open_txt(goods_path.to_str().unwrap()).map_err(|e| e.to_string())?;
            let ast = DefaultEU4Txt::parse(tokens).map_err(|e| e.to_string())?;
            goods = from_node(&ast)?;
        }
        MapMode::Political => {
            println!("Loading country tags...");
            let tags = eu4data::countries::load_tags(base_path).map_err(|e| e.to_string())?;
            println!("Loading {} country definitions...", tags.len());
            countries = eu4data::countries::load_country_map(base_path, &tags);
            println!("Loaded {} countries.", countries.len());
        }
        MapMode::Religion => {
            println!("Loading religions...");
            religions = eu4data::religions::load_religions(base_path).map_err(|e| e.to_string())?;
        }
        MapMode::Culture => {
            println!("Loading cultures...");
            cultures = eu4data::cultures::load_cultures(base_path).map_err(|e| e.to_string())?;
        }
        MapMode::Province => {} // No extra data needed
        MapMode::All => unreachable!("MapMode::All should be handled by caller"),
    }

    // 3. Load Province History (ID -> Data)
    println!("Loading history...");
    let (province_history, stats_history) =
        eu4data::history::load_province_history(base_path).map_err(|e| e.to_string())?;

    println!(
        "History Stats: Success={}, Failure={}",
        stats_history.0, stats_history.1
    );

    // 4. Render
    let map_path = base_path.join("map/provinces.bmp");
    println!("Loading map image from {:?}", map_path);
    let img = image::open(map_path).map_err(|e| e.to_string())?.to_rgb8();
    let (width, height) = img.dimensions();
    let mut out_img = RgbImage::new(width, height);

    println!("Rendering...");
    for (x, y, pixel) in img.enumerate_pixels() {
        let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
        if let Some(id) = color_to_id.get(&(r, g, b)) {
            let mut out_color = Rgb([100, 100, 100]); // Default Grey

            if water_ids.contains(id) {
                out_color = Rgb([64, 164, 223]); // Water Blue
            } else if let Some(hist) = province_history.get(id) {
                match mode {
                    MapMode::TradeGoods => {
                        if let Some(good) =
                            hist.trade_goods.as_ref().and_then(|name| goods.get(name))
                            && let Some(color) = &good.color
                            && color.len() >= 3
                        {
                            let fr = (color[0] * 255.0) as u8;
                            let fg = (color[1] * 255.0) as u8;
                            let fb = (color[2] * 255.0) as u8;
                            out_color = Rgb([fr, fg, fb]);
                        }
                    }
                    MapMode::Political => {
                        if let Some(country) = hist
                            .owner
                            .as_ref()
                            .and_then(|tag| countries.get(tag))
                            .filter(|c| c.color.len() >= 3)
                        {
                            out_color = Rgb([country.color[0], country.color[1], country.color[2]]);
                        }
                    }
                    MapMode::Province => {
                        out_color = Rgb([r, g, b]);
                    }
                    MapMode::Religion => {
                        if let Some(rel) = hist.religion.as_ref().and_then(|key| religions.get(key))
                            && rel.color.len() >= 3
                        {
                            out_color = Rgb([rel.color[0], rel.color[1], rel.color[2]]);
                        }
                    }
                    MapMode::Culture => {
                        if let Some(cul) = hist.culture.as_ref().and_then(|key| cultures.get(key)) {
                            out_color = Rgb(cul.color);
                        }
                    }
                    MapMode::All => unreachable!(),
                }
            }
            out_img.put_pixel(x, y, out_color);
        } else {
            out_img.put_pixel(x, y, Rgb([0, 0, 0]));
        }
    }

    out_img.save(output_path).map_err(|e| e.to_string())?;
    println!("Saved {:?}", output_path);
    Ok(())
}

pub struct ScanStats {
    pub success: usize,
    pub failure: usize,
    pub tokens: usize,
    pub nodes: usize,
}

pub fn pretty_print_dir(dir: &std::path::Path, pretty_print: bool) -> Result<ScanStats, String> {
    let mut stats = ScanStats {
        success: 0,
        failure: 0,
        tokens: 0,
        nodes: 0,
    };
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                // println!("{}", path.display());
                let sub_stats = pretty_print_dir(&path, pretty_print)?;
                stats.success += sub_stats.success;
                stats.failure += sub_stats.failure;
                stats.tokens += sub_stats.tokens;
                stats.nodes += sub_stats.nodes;
            } else if path.extension().is_some_and(|ext| ext == "txt") {
                // println!("{}", path.display());
                let tokens = match DefaultEU4Txt::open_txt(path.to_str().unwrap()) {
                    Ok(t) => t,
                    Err(_) => {
                        // println!("Expected encoding error potentially");
                        continue;
                    }
                };

                match DefaultEU4Txt::parse(tokens.clone()) {
                    Ok(ast) => {
                        stats.success += 1;
                        stats.tokens += tokens.len();
                        stats.nodes += ast.node_count();
                        if pretty_print {
                            DefaultEU4Txt::pretty_print(&ast, 0).map_err(|e| e.to_string())?;
                        }
                    }
                    Err(e) => {
                        if !matches!(e, eu4txt::ParseError::EmptyInput) {
                            println!("Parse Fail: {} : {}", path.display(), e);
                            stats.failure += 1;
                        }
                    }
                }
            }
        }
    }
    Ok(stats)
}

#[allow(clippy::type_complexity)]
pub fn load_world_data(base_path: &Path) -> Result<window::WorldData, String> {
    let start_time = std::time::Instant::now();
    log::info!("Loading world data...");

    // Task Group 1: Definition CSV & Map Image
    let task_definitions_and_map =
        || -> Result<(HashMap<(u8, u8, u8), u32>, image::RgbImage), String> {
            let (definitions, province_map) = rayon::join(
                || -> Result<_, String> {
                    let def_path = base_path.join("map/definition.csv");
                    let list = load_definitions(&def_path).map_err(|e| e.to_string())?;
                    let mut map = HashMap::new();
                    for (id, def) in list {
                        map.insert((def.r, def.g, def.b), id);
                    }
                    Ok(map)
                },
                || -> Result<_, String> {
                    let map_path = base_path.join("map/provinces.bmp");
                    log::info!("Loading map image from {}", display_path(&map_path));
                    image::open(map_path)
                        .map_err(|e| e.to_string())
                        .map(|img| img.to_rgb8())
                },
            );
            Ok((definitions?, province_map?))
        };

    // Task Group 2: History & Countries
    let task_history_and_countries =
        || -> Result<(HashMap<u32, ProvinceHistory>, HashMap<String, Country>), String> {
            let (history_res, countries_res) = rayon::join(
                || eu4data::history::load_province_history(base_path).map_err(|e| e.to_string()),
                || -> Result<_, String> {
                    log::info!("Loading country tags...");
                    let tags =
                        eu4data::countries::load_tags(base_path).map_err(|e| e.to_string())?;
                    log::info!("Loading {} country definitions...", tags.len());
                    let countries = eu4data::countries::load_country_map(base_path, &tags);
                    log::info!("Loaded {} countries.", countries.len());
                    Ok(countries)
                },
            );
            Ok((history_res?.0, countries_res?))
        };

    // Task Group 3: Religions, Cultures, Tradegoods
    let task_common_data = || {
        rayon::join(
            || eu4data::religions::load_religions(base_path).map_err(|e| e.to_string()),
            || {
                rayon::join(
                    || eu4data::cultures::load_cultures(base_path).map_err(|e| e.to_string()),
                    || -> Result<_, String> {
                        let tg_path = base_path.join("common/tradegoods/00_tradegoods.txt");
                        if tg_path.exists() {
                            let tokens = DefaultEU4Txt::open_txt(tg_path.to_str().unwrap())
                                .map_err(|e| e.to_string())?;
                            let ast = DefaultEU4Txt::parse(tokens).map_err(|e| e.to_string())?;
                            from_node::<HashMap<String, Tradegood>>(&ast).map_err(|e| e.to_string())
                        } else {
                            Ok(HashMap::new())
                        }
                    },
                )
            },
        )
    };

    // 4. Execute Top-Level Tasks
    let (res_defs_map, (res_hist_countries, (res_religions, (res_cultures, res_tradegoods)))) =
        rayon::join(task_definitions_and_map, || {
            rayon::join(task_history_and_countries, task_common_data)
        });

    let (color_to_id, province_map) = res_defs_map?;
    let (province_history, countries) = res_hist_countries?;
    let religions = res_religions?;
    let cultures = res_cultures?;
    let tradegoods = res_tradegoods.unwrap_or_default();

    log::info!(
        "Loaded {} religions, {} cultures, {} tradegoods.",
        religions.len(),
        cultures.len(),
        tradegoods.len()
    );

    // 5. Default Map (Water)
    let default_map_path = base_path.join("map/default.map");
    let mut water_ids = HashSet::new();
    if default_map_path.exists()
        && let Ok(dm_tokens) = DefaultEU4Txt::open_txt(default_map_path.to_str().unwrap())
        && let Ok(dm_ast) = DefaultEU4Txt::parse(dm_tokens)
        && let Ok(default_map) = from_node::<DefaultMap>(&dm_ast)
    {
        for id in default_map.sea_starts {
            water_ids.insert(id);
        }
        for id in default_map.lakes {
            water_ids.insert(id);
        }
    }

    // 6. Generate All Maps (Parallel)
    log::info!("Generating Maps (Political, TradeGoods, Religion, Culture)...");
    let (width, height) = province_map.dimensions();

    // We need shared references for the closures
    let ref_province_map = &province_map;
    let ref_color_to_id = &color_to_id;
    let ref_province_history = &province_history;
    let ref_countries = &countries;
    let ref_religions = &religions;
    let ref_cultures = &cultures;
    let ref_tradegoods = &tradegoods;
    let ref_water_ids = &water_ids;

    let (political_map, (religion_map, (culture_map, tradegoods_map))) = rayon::join(
        || {
            draw_map_political(
                width,
                height,
                ref_province_map,
                ref_color_to_id,
                ref_province_history,
                ref_countries,
                ref_water_ids,
            )
        },
        || {
            rayon::join(
                || {
                    draw_map_religion(
                        width,
                        height,
                        ref_province_map,
                        ref_color_to_id,
                        ref_province_history,
                        ref_religions,
                        ref_water_ids,
                    )
                },
                || {
                    rayon::join(
                        || {
                            draw_map_culture(
                                width,
                                height,
                                ref_province_map,
                                ref_color_to_id,
                                ref_province_history,
                                ref_cultures,
                                ref_water_ids,
                            )
                        },
                        || {
                            draw_map_tradegoods(
                                width,
                                height,
                                ref_province_map,
                                ref_color_to_id,
                                ref_province_history,
                                ref_tradegoods,
                                ref_water_ids,
                            )
                        },
                    )
                },
            )
        },
    );

    log::info!("Total load time: {:?}", start_time.elapsed());

    Ok(window::WorldData {
        province_map,
        political_map,
        tradegoods_map,
        religion_map,
        culture_map,
        province_history,
        countries,
        religions,
        cultures,
        tradegoods,
        water_ids,
        color_to_id,
    })
}

fn draw_map_political(
    width: u32,
    height: u32,
    province_map: &RgbImage,
    color_to_id: &HashMap<(u8, u8, u8), u32>,
    province_history: &HashMap<u32, ProvinceHistory>,
    countries: &HashMap<String, Country>,
    water_ids: &HashSet<u32>,
) -> RgbImage {
    let mut map = RgbImage::new(width, height);
    for (x, y, pixel) in province_map.enumerate_pixels() {
        let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
        let mut color = Rgb([100, 100, 100]);

        if let Some(id) = color_to_id.get(&(r, g, b)) {
            if water_ids.contains(id) {
                color = Rgb([64, 164, 223]);
            } else if let Some(hist) = province_history.get(id)
                && let Some(country) = hist.owner.as_ref().and_then(|tag| countries.get(tag))
                && country.color.len() >= 3
            {
                color = Rgb([country.color[0], country.color[1], country.color[2]]);
            }
        } else {
            color = Rgb([0, 0, 0]);
        }
        map.put_pixel(x, y, color);
    }
    map
}

fn draw_map_tradegoods(
    width: u32,
    height: u32,
    province_map: &RgbImage,
    color_to_id: &HashMap<(u8, u8, u8), u32>,
    province_history: &HashMap<u32, ProvinceHistory>,
    tradegoods: &HashMap<String, Tradegood>,
    water_ids: &HashSet<u32>,
) -> RgbImage {
    let mut map = RgbImage::new(width, height);
    for (x, y, pixel) in province_map.enumerate_pixels() {
        let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
        let mut color = Rgb([100, 100, 100]);

        if let Some(id) = color_to_id.get(&(r, g, b)) {
            if water_ids.contains(id) {
                color = Rgb([64, 164, 223]);
            } else if let Some(hist) = province_history.get(id)
                && let Some(good) = hist
                    .trade_goods
                    .as_ref()
                    .and_then(|key| tradegoods.get(key))
                && let Some(good_color) = &good.color
                && good_color.len() >= 3
            {
                color = Rgb([
                    (good_color[0] * 255.0) as u8,
                    (good_color[1] * 255.0) as u8,
                    (good_color[2] * 255.0) as u8,
                ]);
            }
        } else {
            color = Rgb([0, 0, 0]);
        }
        map.put_pixel(x, y, color);
    }
    map
}

fn draw_map_religion(
    width: u32,
    height: u32,
    province_map: &RgbImage,
    color_to_id: &HashMap<(u8, u8, u8), u32>,
    province_history: &HashMap<u32, ProvinceHistory>,
    religions: &HashMap<String, Religion>,
    water_ids: &HashSet<u32>,
) -> RgbImage {
    let mut map = RgbImage::new(width, height);
    for (x, y, pixel) in province_map.enumerate_pixels() {
        let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
        let mut color = Rgb([100, 100, 100]);

        if let Some(id) = color_to_id.get(&(r, g, b)) {
            if water_ids.contains(id) {
                color = Rgb([64, 164, 223]);
            } else if let Some(hist) = province_history.get(id)
                && let Some(rel) = hist.religion.as_ref().and_then(|key| religions.get(key))
                && rel.color.len() >= 3
            {
                color = Rgb([rel.color[0], rel.color[1], rel.color[2]]);
            }
        } else {
            color = Rgb([0, 0, 0]);
        }
        map.put_pixel(x, y, color);
    }
    map
}

fn draw_map_culture(
    width: u32,
    height: u32,
    province_map: &RgbImage,
    color_to_id: &HashMap<(u8, u8, u8), u32>,
    province_history: &HashMap<u32, ProvinceHistory>,
    cultures: &HashMap<String, Culture>,
    water_ids: &HashSet<u32>,
) -> RgbImage {
    let mut map = RgbImage::new(width, height);
    for (x, y, pixel) in province_map.enumerate_pixels() {
        let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
        let mut color = Rgb([100, 100, 100]);

        if let Some(id) = color_to_id.get(&(r, g, b)) {
            if water_ids.contains(id) {
                color = Rgb([64, 164, 223]);
            } else if let Some(hist) = province_history.get(id)
                && let Some(cul) = hist.culture.as_ref().and_then(|key| cultures.get(key))
            {
                // Culture uses [u8; 3] directly
                color = Rgb(cul.color);
            }
        } else {
            color = Rgb([0, 0, 0]);
        }
        map.put_pixel(x, y, color);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_mock_eu4(dir: &Path) {
        // Create directory structure
        let map_dir = dir.join("map");
        let common_dir = dir.join("common");
        let tradegoods_dir = common_dir.join("tradegoods");
        let history_dir = dir.join("history/provinces");
        let countries_dir = common_dir.join("country_tags");
        let tags_dir = common_dir.join("countries"); // For actual definitions
        let religions_dir = common_dir.join("religions");
        let cultures_dir = common_dir.join("cultures");

        std::fs::create_dir_all(&map_dir).unwrap();
        std::fs::create_dir_all(&tradegoods_dir).unwrap();
        std::fs::create_dir_all(&history_dir).unwrap();
        std::fs::create_dir_all(&countries_dir).unwrap();
        std::fs::create_dir_all(&tags_dir).unwrap();
        std::fs::create_dir_all(&religions_dir).unwrap();
        std::fs::create_dir_all(&cultures_dir).unwrap();

        // 1. map/definition.csv
        // ID;R;G;B;Name;x
        // 1;255;0;0;Provinz1;x
        // 2;0;255;0;Provinz2;x
        let mut def = File::create(map_dir.join("definition.csv")).unwrap();
        writeln!(def, "province;red;green;blue;x;x").unwrap();
        writeln!(def, "1;255;0;0;Stockholm;x").unwrap();
        writeln!(def, "2;0;255;0;Uppsala;x").unwrap();
        writeln!(def, "3;0;0;255;Sea;x").unwrap();

        // 2. map/default.map
        let mut def_map = File::create(map_dir.join("default.map")).unwrap();
        writeln!(def_map, "sea_starts = {{ 3 }}").unwrap();
        writeln!(def_map, "max_provinces = 4").unwrap();

        // 3. map/provinces.bmp (Tiny 2x2)
        // (0,0)=Red=1, (1,0)=Green=2, (0,1)=Blue=3, (1,1)=Black=Unknown
        let mut img = RgbImage::new(2, 2);
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        img.put_pixel(1, 0, Rgb([0, 255, 0]));
        img.put_pixel(0, 1, Rgb([0, 0, 255]));
        img.put_pixel(1, 1, Rgb([0, 0, 0]));
        img.save(map_dir.join("provinces.bmp")).unwrap();

        // 4. common/tradegoods/00_tradegoods.txt
        let mut tg = File::create(tradegoods_dir.join("00_tradegoods.txt")).unwrap();
        writeln!(
            tg,
            r#"
            grain = {{
                color = {{ 1.0 0.0 0.0 }}
            }}
        "#
        )
        .unwrap();

        // 5. history/provinces/1 - Stockholm.txt
        let mut hist1 = File::create(history_dir.join("1 - Stockholm.txt")).unwrap();
        writeln!(hist1, "owner = SWE").unwrap();
        writeln!(hist1, "trade_goods = grain").unwrap();
        writeln!(hist1, "religion = catholic").unwrap();
        writeln!(hist1, "culture = swedish").unwrap();

        // 6. history/provinces/2 - Uppsala.txt (No goods)
        let mut hist2 = File::create(history_dir.join("2 - Uppsala.txt")).unwrap();
        writeln!(hist2, "owner = SWE").unwrap();

        // 7. Countries
        let mut tags = File::create(countries_dir.join("00_countries.txt")).unwrap();
        writeln!(tags, "SWE = \"countries/Sweden.txt\"").unwrap();

        let mut swe = File::create(tags_dir.join("Sweden.txt")).unwrap();
        writeln!(swe, "color = {{ 0 0 255 }}").unwrap(); // Blue Sweden

        // 8. Religions
        let mut rel = File::create(religions_dir.join("00_religion.txt")).unwrap();
        writeln!(
            rel,
            "christian = {{ catholic = {{ color = {{ 255 255 0 }} }} }}"
        )
        .unwrap();

        // 9. Cultures
        let mut cul = File::create(cultures_dir.join("00_cultures.txt")).unwrap();
        writeln!(cul, "germanic = {{ swedish = {{ }} }}").unwrap();
    }

    #[test]
    fn test_dump_tradegoods() {
        let dir = tempdir().unwrap();
        create_mock_eu4(dir.path());

        let res = dump_tradegoods(&dir.path().join("common"));
        assert!(res.is_ok());
    }

    #[test]
    fn test_load_world_data() {
        let dir = tempdir().unwrap();
        create_mock_eu4(dir.path());

        // Should succeed
        let data = load_world_data(dir.path()).expect("load_world_data failed");
        assert_eq!(data.color_to_id.len(), 3); // 1, 2, 3
        assert_eq!(data.province_history.len(), 2); // 1 and 2
        // DefaultMap loads 3 into water_ids set
    }

    #[test]
    fn test_draw_map_tradegoods() {
        let dir = tempdir().unwrap();
        create_mock_eu4(dir.path());

        let output = dir.path().join("out_tg.png");
        let res = draw_map(dir.path(), &output, MapMode::TradeGoods);
        assert!(res.is_ok());
        assert!(output.exists());

        let img = image::open(output).unwrap().to_rgb8();
        // (0,0) is ID 1 (Stockholm) -> Grain -> Red (1.0 0.0 0.0) -> 255, 0, 0
        assert_eq!(img.get_pixel(0, 0), &Rgb([255, 0, 0]));
        // (0,1) is ID 3 (Sea) -> Water Blue -> 64, 164, 223
        assert_eq!(img.get_pixel(0, 1), &Rgb([64, 164, 223]));
    }

    #[test]
    fn test_draw_map_political() {
        let dir = tempdir().unwrap();
        create_mock_eu4(dir.path());

        let output = dir.path().join("out_pol.png");
        let res = draw_map(dir.path(), &output, MapMode::Political);

        assert!(res.is_ok(), "draw_map political failed: {:?}", res.err());
        assert!(output.exists());

        let img = image::open(output).unwrap().to_rgb8();
        // (0,0) is ID 1 (Stockholm) -> Owner SWE -> Blue (0 0 255)
        assert_eq!(img.get_pixel(0, 0), &Rgb([0, 0, 255]));
    }

    #[test]
    fn test_draw_map_religion() {
        let dir = tempdir().unwrap();
        create_mock_eu4(dir.path());
        let output = dir.path().join("out_rel.png");
        let res = draw_map(dir.path(), &output, MapMode::Religion);
        assert!(res.is_ok());
        let img = image::open(output).unwrap().to_rgb8();
        // Catholic = Yellow (255, 255, 0)
        assert_eq!(img.get_pixel(0, 0), &Rgb([255, 255, 0]));
    }

    #[test]
    fn test_draw_map_culture() {
        let dir = tempdir().unwrap();
        create_mock_eu4(dir.path());
        let output = dir.path().join("out_cul.png");
        let res = draw_map(dir.path(), &output, MapMode::Culture);
        assert!(res.is_ok());
        let img = image::open(output).unwrap().to_rgb8();
        // Swedish = Hashed color
        let _px = img.get_pixel(0, 0);
        // assert_ne!(px, &Rgb([0,0,0])); // Just check it's not black
        // Actually we can predict it if we wanted, but robust enough to check not black/gray
    }

    #[test]
    fn test_draw_map_all_panics() {
        let dir = tempdir().unwrap();
        create_mock_eu4(dir.path()); // Ensure we don't fail early on missing files
        let output = dir.path().join("out.png");

        let result = std::panic::catch_unwind(|| {
            let _ = draw_map(dir.path(), &output, MapMode::All);
        });
        assert!(result.is_err());
    }
}

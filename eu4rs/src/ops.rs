use crate::args::MapMode;
use crate::window;
use eu4data::{
    Tradegood,
    map::{DefaultMap, load_definitions},
};
use eu4txt::{DefaultEU4Txt, EU4Txt, from_node};
use image::{Rgb, RgbImage};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub fn dump_tradegoods(base_path: &std::path::Path) -> Result<(), String> {
    let path = base_path.join("tradegoods/00_tradegoods.txt");
    println!("Loading {:?}", path);

    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| e.to_string())?;
    let ast = DefaultEU4Txt::parse(tokens)?;
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
    let dm_ast = DefaultEU4Txt::parse(dm_tokens)?;
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

    match mode {
        MapMode::TradeGoods => {
            let goods_path = base_path.join("common/tradegoods/00_tradegoods.txt");
            println!("Loading trade goods from {:?}", goods_path);
            let tokens =
                DefaultEU4Txt::open_txt(goods_path.to_str().unwrap()).map_err(|e| e.to_string())?;
            let ast = DefaultEU4Txt::parse(tokens)?;
            goods = from_node(&ast)?;
        }
        MapMode::Political => {
            println!("Loading country tags...");
            let tags = eu4data::countries::load_tags(base_path).map_err(|e| e.to_string())?;
            println!("Loading {} country definitions...", tags.len());
            countries = eu4data::countries::load_country_map(base_path, &tags);
            println!("Loaded {} countries.", countries.len());
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
                        if let Some(good) = hist
                            .trade_goods
                            .as_ref()
                            .and_then(|name| goods.get(name))
                            .filter(|g| g.color.len() >= 3)
                        {
                            let fr = (good.color[0] * 255.0) as u8;
                            let fg = (good.color[1] * 255.0) as u8;
                            let fb = (good.color[2] * 255.0) as u8;
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
                            DefaultEU4Txt::pretty_print(&ast, 0)?;
                        }
                    }
                    Err(e) => {
                        if e != "NoTokens" {
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

pub fn load_world_data(base_path: &Path) -> Result<window::WorldData, String> {
    println!("Loading world data...");

    // 1. Definitions
    let def_path = base_path.join("map/definition.csv");
    let definitions = load_definitions(&def_path).map_err(|e| e.to_string())?;
    let mut color_to_id = HashMap::new();
    for (id, def) in &definitions {
        color_to_id.insert((def.r, def.g, def.b), *id);
    }

    // 2. History
    let (province_history, _) =
        eu4data::history::load_province_history(base_path).map_err(|e| e.to_string())?;

    // 3. Map Image
    let map_path = base_path.join("map/provinces.bmp");
    println!("Loading map image from {:?}", map_path);
    let province_map = image::open(map_path).map_err(|e| e.to_string())?.to_rgb8();

    // 4. Countries
    println!("Loading country tags...");
    let tags = eu4data::countries::load_tags(base_path).map_err(|e| e.to_string())?;
    println!("Loading {} country definitions...", tags.len());
    let countries = eu4data::countries::load_country_map(base_path, &tags);
    println!("Loaded {} countries.", countries.len());

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

    // 6. Generate Political Map
    println!("Generating Political Map...");
    let (width, height) = province_map.dimensions();
    let mut political_map = RgbImage::new(width, height);

    for (x, y, pixel) in province_map.enumerate_pixels() {
        let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
        if let Some(id) = color_to_id.get(&(r, g, b)) {
            let mut out_color = Rgb([100, 100, 100]); // Gray (Unowned/Wasteland)

            if water_ids.contains(id) {
                out_color = Rgb([64, 164, 223]); // Water Blue
            } else if let Some(hist) = province_history.get(id)
                && let Some(country) = hist.owner.as_ref().and_then(|tag| countries.get(tag))
                && country.color.len() >= 3
            {
                out_color = Rgb([country.color[0], country.color[1], country.color[2]]);
            }
            political_map.put_pixel(x, y, out_color);
        } else {
            political_map.put_pixel(x, y, Rgb([0, 0, 0]));
        }
    }

    Ok(window::WorldData {
        province_map,
        political_map,
        color_to_id,
        province_history,
        countries,
    })
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

        std::fs::create_dir_all(&map_dir).unwrap();
        std::fs::create_dir_all(&tradegoods_dir).unwrap();
        std::fs::create_dir_all(&history_dir).unwrap();
        std::fs::create_dir_all(&countries_dir).unwrap();
        std::fs::create_dir_all(&tags_dir).unwrap();

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

        // 6. history/provinces/2 - Uppsala.txt (No goods)
        let mut hist2 = File::create(history_dir.join("2 - Uppsala.txt")).unwrap();
        writeln!(hist2, "owner = SWE").unwrap();

        // 7. Countries
        let mut tags = File::create(countries_dir.join("00_countries.txt")).unwrap();
        writeln!(tags, "SWE = \"countries/Sweden.txt\"").unwrap();

        let mut swe = File::create(tags_dir.join("Sweden.txt")).unwrap();
        writeln!(swe, "color = {{ 0 0 255 }}").unwrap(); // Blue Sweden
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

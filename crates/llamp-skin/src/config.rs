//! `region.txt`, `viscolor.txt`, and `pledit.txt`.

use crate::{PlaylistColors, Polygon, Regions, Rgb};

pub fn default_vis_colors() -> [Rgb; 24] {
    let mut colors = [Rgb { r: 0, g: 0, b: 0 }; 24];
    colors[1] = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let ramp = [
        (0, 255),
        (17, 238),
        (34, 221),
        (51, 204),
        (68, 187),
        (85, 170),
        (102, 153),
        (119, 136),
        (136, 119),
        (153, 102),
        (170, 85),
        (187, 68),
        (204, 51),
        (221, 34),
        (238, 17),
        (255, 0),
    ];
    for (i, (r, g)) in ramp.into_iter().enumerate() {
        colors[2 + i] = Rgb { r, g, b: 0 };
    }
    for slot in 18..23 {
        colors[slot] = Rgb { r: 0, g: 255, b: 0 };
    }
    colors[23] = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    colors
}

pub fn default_playlist_colors() -> PlaylistColors {
    PlaylistColors {
        normal: Rgb { r: 0, g: 255, b: 0 },
        current: Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        normal_bg: Rgb { r: 0, g: 0, b: 0 },
        selected_bg: Rgb {
            r: 0,
            g: 0,
            b: 0xC0,
        },
        mb_fg: Rgb { r: 0, g: 255, b: 0 },
        mb_bg: Rgb { r: 0, g: 0, b: 0 },
        requested_font: "llamp-pixel".into(),
    }
}

pub fn parse_viscolor(text: &str, defects: &mut Vec<String>) -> [Rgb; 24] {
    let mut colors = default_vis_colors();
    let mut index = 0usize;
    for line in text.lines() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split(',').map(str::trim);
        let Some(r) = parts.next().and_then(parse_u8) else {
            defects.push("viscolor.txt line did not parse".into());
            continue;
        };
        let Some(g) = parts.next().and_then(parse_u8) else {
            defects.push("viscolor.txt line did not parse".into());
            continue;
        };
        let Some(b) = parts.next().and_then(parse_u8) else {
            defects.push("viscolor.txt line did not parse".into());
            continue;
        };
        if index < 24 {
            colors[index] = Rgb { r, g, b };
            index += 1;
        }
    }
    if index < 24 {
        defects.push(format!("viscolor.txt has {index} colors, expected 24"));
    }
    colors
}

pub fn parse_pledit(text: &str) -> PlaylistColors {
    let mut colors = default_playlist_colors();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key.to_ascii_lowercase().as_str() {
            "normal" => colors.normal = parse_hex(value).unwrap_or(colors.normal),
            "current" => colors.current = parse_hex(value).unwrap_or(colors.current),
            "normalbg" => colors.normal_bg = parse_hex(value).unwrap_or(colors.normal_bg),
            "selectedbg" => colors.selected_bg = parse_hex(value).unwrap_or(colors.selected_bg),
            "mbfg" => colors.mb_fg = parse_hex(value).unwrap_or(colors.mb_fg),
            "mbbg" => colors.mb_bg = parse_hex(value).unwrap_or(colors.mb_bg),
            "font" => colors.requested_font = value.to_string(),
            _ => {}
        }
    }
    colors
}

pub fn parse_region(text: &str, defects: &mut Vec<String>) -> Regions {
    let mut regions = Regions {
        normal: Vec::new(),
        window_shade: Vec::new(),
        equalizer: Vec::new(),
        equalizer_ws: Vec::new(),
    };
    let mut section = String::new();
    let mut num_points: Vec<i32> = Vec::new();
    let mut points: Vec<i32> = Vec::new();
    let flush = |section: &str,
                 num_points: &mut Vec<i32>,
                 points: &mut Vec<i32>,
                 regions: &mut Regions,
                 defects: &mut Vec<String>| {
        if section.is_empty() {
            num_points.clear();
            points.clear();
            return;
        }
        let Some((width, height)) = section_size(section) else {
            num_points.clear();
            points.clear();
            return;
        };
        let mut cursor = 0usize;
        let mut polygons = Vec::new();
        for count in num_points.drain(..) {
            if count < 3 {
                cursor = cursor.saturating_add((count.max(0) as usize) * 2);
                continue;
            }
            let n = count as usize;
            let mut poly = Vec::new();
            for _ in 0..n {
                if cursor + 1 >= points.len() {
                    break;
                }
                let (x, y) = (points[cursor], points[cursor + 1]);
                cursor += 2;
                let cx = x.clamp(0, width - 1);
                let cy = y.clamp(0, height - 1);
                if cx != x || cy != y {
                    defects.push(format!("region point clamped in [{section}]"));
                }
                poly.push((cx, cy));
            }
            if poly.len() >= 3 {
                polygons.push(Polygon { points: poly });
            }
        }
        match section.to_ascii_lowercase().as_str() {
            "normal" => regions.normal = polygons,
            "windowshade" => regions.window_shade = polygons,
            "equalizer" => regions.equalizer = polygons,
            "equalizerws" => regions.equalizer_ws = polygons,
            _ => {}
        }
        points.clear();
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            flush(
                &section,
                &mut num_points,
                &mut points,
                &mut regions,
                defects,
            );
            section = name.to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let nums = parse_ints(value);
        match key.trim().to_ascii_lowercase().as_str() {
            "numpoints" => num_points.extend(nums),
            "pointlist" => points.extend(nums),
            _ => {}
        }
    }
    flush(
        &section,
        &mut num_points,
        &mut points,
        &mut regions,
        defects,
    );
    regions
}

fn section_size(section: &str) -> Option<(i32, i32)> {
    match section.to_ascii_lowercase().as_str() {
        "normal" | "equalizer" => Some((275, 116)),
        "windowshade" | "equalizerws" => Some((275, 14)),
        _ => None,
    }
}

fn parse_ints(value: &str) -> Vec<i32> {
    value
        .split(',')
        .filter_map(|part| part.trim().parse::<i32>().ok())
        .collect()
}

fn strip_comment(line: &str) -> &str {
    line.split("//")
        .next()
        .unwrap_or(line)
        .split(';')
        .next()
        .unwrap_or(line)
}

fn parse_u8(text: &str) -> Option<u8> {
    text.parse::<u16>()
        .ok()
        .filter(|n| *n <= 255)
        .map(|n| n as u8)
}

fn parse_hex(text: &str) -> Option<Rgb> {
    let text = text.strip_prefix('#').unwrap_or(text);
    if text.len() != 6 {
        return None;
    }
    Some(Rgb {
        r: u8::from_str_radix(&text[0..2], 16).ok()?,
        g: u8::from_str_radix(&text[2..4], 16).ok()?,
        b: u8::from_str_radix(&text[4..6], 16).ok()?,
    })
}

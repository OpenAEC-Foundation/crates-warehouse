//! Route-A go/no-go: kan Ã“NZE parser/renderer (open-pdf-render) het
//! 5M-ops-blad MV-03 aan, en met welke tijden/geheugen?
//!   1. load (lopdf-parse)
//!   2. extract_draw_commands (display-list â€” kern van route A)
//!   3. render_page @0.3 (zelfde workload als de PDFium-probe: 11,6 s ref)
use std::time::Instant;

const PDF: &str = "C:/Users/rickd/Documents/GitHub/verification-files/PDF-bestanden/MV-03_Mechanische ventilatie, 3e verdieping ontwerp ACH van 1,5 naar 2,0.pdf";

fn main() {
    let bytes = std::fs::read(PDF).expect("read pdf");
    println!("bestand: {:.1} MB", bytes.len() as f64 / 1048576.0);

    let t0 = Instant::now();
    let doc = match openaec_pdf_v1::DocumentHandle::load(&bytes) {
        Ok(d) => d,
        Err(e) => { println!("LOAD FOUT: {e:?}"); return; }
    };
    println!("load/parse: {} ms", t0.elapsed().as_millis());

    let t1 = Instant::now();
    match doc.extract_draw_commands(0, 0) {
        Ok(buf) => println!(
            "extract_draw_commands: {} ms, buffer {:.1} MB",
            t1.elapsed().as_millis(),
            buf.len() as f64 / 1048576.0
        ),
        Err(e) => println!("EXTRACT FOUT na {} ms: {e:?}", t1.elapsed().as_millis()),
    }

    // â”€â”€ Route A fase 2: chunk-index + parallelle tegels â”€â”€
    let t1b = Instant::now();
    let buf2 = doc.extract_draw_commands(0, 0).expect("extract voor tiles");
    let scene = openaec_pdf_v1::tile_render::TileScene::build(buf2.into_bytes()).expect("scene");
    println!("tile-index: {} ms, {} chunks", t1b.elapsed().as_millis(), scene.chunk_count());

    // Zelfde-engine-verificatie op echte data: 1 mega-tegel vs 512px-tegels.
    let tref = Instant::now();
    let one = scene.render_full_parallel(0.3, 1_000_000);
    println!("replayer 1-tegel @0.3: {} ms ({}x{})", tref.elapsed().as_millis(), one.width, one.height);
    for tile_px in [512u32, 1024] {
        let tt = Instant::now();
        let tiled = scene.render_full_parallel(0.3, tile_px);
        let diff = one.rgba.iter().zip(tiled.rgba.iter()).filter(|(a, b)| a != b).count();
        println!(
            "tiled @0.3 tegel={}px: {} ms, byte-diff vs 1-tegel: {} ({:.4}%)",
            tile_px, tt.elapsed().as_millis(), diff,
            100.0 * diff as f64 / one.rgba.len() as f64
        );
        if diff > 0 {
            // Diagnose: waar en hoe groot zijn de verschillen?
            let w = one.width as usize;
            let mut on_boundary = 0usize;
            let mut max_delta = 0i32;
            let mut npx = 0usize;
            let mut first: Vec<(usize, usize)> = Vec::new();
            for (i, (a, b)) in one.rgba.chunks_exact(4).zip(tiled.rgba.chunks_exact(4)).enumerate() {
                if a == b { continue; }
                npx += 1;
                let x = i % w;
                let y = i / w;
                let tb = tile_px as usize;
                if x % tb <= 1 || x % tb >= tb - 2 || y % tb <= 1 || y % tb >= tb - 2 { on_boundary += 1; }
                for c in 0..4 {
                    max_delta = max_delta.max((a[c] as i32 - b[c] as i32).abs());
                }
                if first.len() < 8 { first.push((x, y)); }
            }
            println!("   diff-pixels: {}  op tegelrand: {}  max-kanaal-delta: {}  eerste: {:?}",
                npx, on_boundary, max_delta, first);
        }
        if tile_px == 512 {
            if let Some(pm) = tiny_skia::Pixmap::from_vec(
                tiled.rgba.clone(),
                tiny_skia::IntSize::from_wh(tiled.width, tiled.height).unwrap(),
            ) {
                let out = std::env::temp_dir().join("mv03_tiled_render.png");
                let _ = pm.save_png(&out);
                println!("tiled png: {}", out.display());
            }
        }
    }
    // Halveringsexperiment: tegel (512,0) 512x512 mÃ©t en zonder culling,
    // vergeleken met dezelfde uitsnede uit de 1-tegel-render.
    {
        let with_cull = scene.render_tile_debug(0.3, 512, 0, 500, 512, true);
        let no_cull = scene.render_tile_debug(0.3, 512, 0, 500, 512, false);
        let w_full = one.width as usize;
        let mut d_cull = 0usize;
        let mut d_nocull = 0usize;
        for y in 0..512usize.min(one.height as usize) {
            for x in 0..500usize {
                let fx = 512 + x;
                if fx >= w_full { continue; }
                let fi = (y * w_full + fx) * 4;
                let ti = (y * 500 + x) * 4;
                if one.rgba[fi..fi + 4] != with_cull.data()[ti..ti + 4] { d_cull += 1; }
                if one.rgba[fi..fi + 4] != no_cull.data()[ti..ti + 4] { d_nocull += 1; }
            }
        }
        println!("halvering: diff-met-cull={}  diff-zonder-cull={}", d_cull, d_nocull);
        // Diff-visualisatie: origineel gedimd, verschilpixels felrood.
        let mut vis = one.rgba.clone();
        let wf = one.width as usize;
        let tiled512 = scene.render_full_parallel(0.3, 512);
        let mut worst = (0i32, 0usize, 0usize);
        for (i, (a, b)) in one.rgba.chunks_exact(4).zip(tiled512.rgba.chunks_exact(4)).enumerate() {
            let d: i32 = (0..4).map(|c| (a[c] as i32 - b[c] as i32).abs()).max().unwrap();
            let px = &mut vis[i * 4..i * 4 + 4];
            if d > 0 {
                px[0] = 255; px[1] = 0; px[2] = 0; px[3] = 255;
                if d > worst.0 { worst = (d, i % wf, i / wf); }
            } else {
                px[0] = px[0] / 3 + 170; px[1] = px[1] / 3 + 170; px[2] = px[2] / 3 + 170;
            }
        }
        if let Some(pm) = tiny_skia::Pixmap::from_vec(vis, tiny_skia::IntSize::from_wh(one.width, one.height).unwrap()) {
            let out = std::env::temp_dir().join("mv03_tile_diff.png");
            let _ = pm.save_png(&out);
            println!("diff-vis: {}  worst delta {} @ ({},{})", out.display(), worst.0, worst.1, worst.2);
        }
    }

    // Realistische zoom: volledige pyramide-stap op 3x
    let tz = Instant::now();
    let z = scene.render_full_parallel(3.0, 1024);
    println!("tiled @3.0 tegel=1024px: {} ms ({}x{})", tz.elapsed().as_millis(), z.width, z.height);

    let t2 = Instant::now();
    match doc.render_page(0, 0.3, 0) {
        Ok(p) => {
            println!(
                "render_page @0.3: {} ms ({}x{})",
                t2.elapsed().as_millis(), p.width, p.height
            );
            if let Some(pixmap) = tiny_skia::Pixmap::from_vec(
                p.rgba.clone(),
                tiny_skia::IntSize::from_wh(p.width, p.height).unwrap(),
            ) {
                let out = std::env::temp_dir().join("mv03_probe_render.png");
                let _ = pixmap.save_png(&out);
                println!("png: {}", out.display());
            }
        }
        Err(e) => println!("RENDER FOUT na {} ms: {e:?}", t2.elapsed().as_millis()),
    }
    println!("TOTAAL: {} ms", t0.elapsed().as_millis());
}

//! Die Formungskette gegen die Prüfwerte der Spezifikation.
//!
//! Netz und Daten: CLOs Resizing-Ordner (`CLO_FEMALE` setzt einen anderen).
//! Prüfwerte: `spez/pruefwerte.json`; Toleranzen nach
//! `formung.md` 15. Die Kalibrierung nimmt Sprünge im letzten Bit mit
//! (Hüllpunkte kippen): Der erste E-Wert jeder Stufe gilt bis auf einen
//! Hüllsprung (0,1 mm), danach Endwerte ≤ 0,3 mm, gleicher Ende-Grund,
//! Formungen ± 1, Punkte ≤ 0,1 mm.
//!
//! Die ganze Kette misst bis zu 30-mal, etwa 1 s im Debug-Build je Fall;
//! schneller mit `cargo test --release --test kette_clo`.

use klon3d_koerper::kette::{lies_avs, AvsKopf, Ende, Kalibrierung, Kette};
use klon3d_koerper::massband::{abnahme, rumpfmasse};
use klon3d_koerper::messung::{ANZAHL, NAMEN};
use std::path::PathBuf;

fn ordner() -> Option<PathBuf> {
    let p = std::env::var_os("CLO_FEMALE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female")
        });
    if p.join("ConvertingMat_DETAIL_Human_Bust_TotalHeight.bd")
        .is_file()
    {
        Some(p)
    } else {
        eprintln!("übersprungen: CLO-Resizing-Daten fehlen in {}", p.display());
        None
    }
}

/// Startvektor einer Stufe (`formung.md` 15).
const ERSTE: f64 = 1e-5;
/// Erster E-Wert: `formung.md` 15 verlangt 10⁻⁵ cm. Das hält nur bei
/// bitgleichen Merkmalen: Bizeps springt bei Emma schon zwischen dem
/// float32-Wert aus der `.avs` und seiner Rundung auf 6 Stellen (2·10⁻⁷)
/// um 0,07 mm, die Brust bei `anwenderin_human` um 0,01 mm. Mit dem
/// gerundeten Startvektor trifft die Kette den Prüfwert auf 3·10⁻⁶. Hier
/// deshalb ein Hüllsprung, 0,1 mm.
const ERSTE_E: f64 = 0.01;
const KETTE: f64 = 0.03;
const PUNKT: f64 = 0.01;

/// f0 ist über alle Matrizen gemittelt (Streuung ≤ 1,5·10⁻⁴ cm, `formung.md` 2.7).
const F0: [f64; 3] = [163.949972, 0.0, 9.88562];
const F0_WERTE: [f64; 58] = [
    163.783437, 0.0, 97.720317, 33.294341, 38.366256, 84.68999, 79.950698, 97.504262, 103.419646,
    60.995709, 50.303967, 36.531293, 36.017957, 22.601157, 34.077331, 30.067183, 23.65868,
    15.067991, 139.721894, 135.661087, 4.060807, 119.586134, 113.229624, 106.276114, 93.235943,
    81.361279, 76.708233, 74.064086, 61.42098, 44.998286, 32.643204, 7.623364, 29.182721,
    20.110701, 34.003731, 33.377693, 38.660559, 19.217625, 7.911504, 11.580476, 32.532025,
    36.062877, 13.177956, 25.566343, 18.30608, 25.976445, 70.771297, 55.461452, 74.679077,
    77.181852, 33.32239, 43.859462, 29.567881, 22.607554, 15.287253, 72.761826, 83.160099,
    155.921925,
];
const EMMA_EINGABE: [f64; 58] = [
    175.259995, 0.0, 80.644997, 29.736286, 35.560001, 66.26931, 60.959995, 80.327492, 95.56749,
    54.927498, 44.424545, 33.975677, 33.363335, 21.333187, 41.372318, 25.4, 21.55261, 14.132433,
    149.833862, 145.168365, 4.665497, 128.377701, 122.170822, 113.723618, 101.210976, 87.93824,
    84.137497, 80.5951, 66.341446, 48.383804, 34.98521, 8.318769, 24.388092, 14.11656, 32.62413,
    31.289417, 37.782497, 18.910004, 8.679967, 12.414492, 33.9725, 38.099998, 12.732769, 26.259096,
    16.289873, 25.082502, 67.248688, 58.877625, 77.787498, 72.389999, 31.627136, 40.813782,
    29.586426, 16.580297, 17.795746, 71.403214, 81.685333, 153.088547,
];
const EMMA_NACHBAU: [f64; 58] = [
    175.259667, 0.0, 80.644928, 29.736674, 35.559849, 66.268874, 60.965714, 80.316219, 95.571153,
    54.928023, 44.425236, 33.975611, 33.363829, 21.332591, 41.376762, 25.400198, 21.554861,
    14.14224, 149.834113, 145.168711, 4.665401, 128.377979, 122.171065, 113.723677, 101.210827,
    87.938039, 84.137169, 80.594949, 66.341121, 48.383494, 34.984972, 8.318719, 24.388366,
    14.11641, 32.624666, 31.288484, 37.782454, 18.910006, 8.680318, 12.41468, 33.947696, 38.075257,
    12.732957, 26.259354, 16.289717, 25.085086, 67.248671, 58.877393, 77.787399, 72.440903,
    31.62717, 40.813733, 29.586508, 16.579873, 17.796048, 71.40358, 81.685387, 153.088966,
];
const EMMA_BASE_E: [f64; 3] = [1.054336, 0.091465, 0.009178];
const EMMA_BASE_GEMESSEN: [f64; 58] = [
    175.25974, 0.0, 80.636074, 29.786967, 35.47619, 67.300638, 60.711248, 80.276373, 94.929235,
    55.090958, 44.846977, 34.145465, 33.577472, 21.419914, 41.309453, 25.484446, 21.632474,
    14.118547, 150.071728, 145.555859, 4.51587, 129.060005, 123.58609, 114.267957, 101.310718,
    88.05842, 84.308537, 80.72963, 66.46592, 48.422857, 34.99913, 8.298456, 24.510706, 14.725239,
    32.472211, 31.175717, 37.640323, 18.823097, 8.613837, 12.273732, 33.854417, 37.917565,
    13.149289, 26.677852, 16.137559, 24.818884, 66.875226, 58.704956, 77.528053, 73.744542,
    32.024197, 41.720345, 29.95942, 17.011583, 17.842617, 71.698131, 82.262983, 153.961113,
];
const EMMA_DETAIL_E: [f64; 9] = [
    5.850613, 0.785097, 0.482834, 0.371038, 0.319506, 0.255853, 0.203015, 0.165926, 0.12543,
];
const EMMA_F_DETAIL_START: [f64; 58] = [
    175.259995, 0.0, 8.980256, 5.457744, 5.963221, 8.203697, 7.807688, 8.962561, 9.775863,
    7.411309, 6.696789, 5.843412, 5.794607, 4.628165, 6.427243, 5.039841, 4.651072, 3.757466,
    150.071728, 145.555859, 4.51587, 129.060005, 123.58609, 114.267957, 101.310718, 88.05842,
    84.137497, 80.72963, 66.46592, 48.422857, 34.99913, 8.298456, 24.510706, 14.725239, 32.472211,
    31.175717, 37.782497, 18.823097, 8.613837, 12.273732, 33.9725, 38.099998, 13.149289, 26.677852,
    16.137559, 25.082502, 66.875226, 58.704956, 77.787498, 72.389999, 32.024197, 41.720345,
    29.95942, 17.011583, 17.842617, 71.698131, 82.262983, 153.961113,
];
const EMMA_CLO: [f64; 58] = [
    175.259995, 0.0, 80.644997, 29.736286, 35.560001, 66.26931, 60.959995, 80.327492, 95.56749,
    54.927498, 44.424545, 33.975677, 33.363335, 21.333187, 41.372318, 25.4, 21.55261, 14.132433,
    149.833862, 145.168365, 4.665497, 128.377701, 122.170822, 113.723618, 101.210976, 87.93824,
    84.137497, 80.5951, 66.341446, 48.383804, 34.98521, 8.318769, 24.388092, 14.11656, 32.62413,
    31.289417, 37.782497, 18.910004, 8.679967, 12.414492, 33.9725, 38.099998, 12.732769, 26.259096,
    16.289873, 25.082502, 67.248688, 58.877625, 77.787498, 72.389999, 31.627136, 40.813782,
    29.586426, 16.580297, 17.795746, 71.403214, 81.685333, 153.088547,
];
const HUMAN_EINGABE: [f64; 58] = [
    160.0, 0.0, 92.669998, 31.671263, 37.000004, 76.000008, 74.0, 86.199997, 89.999992, 54.0,
    45.88224, 36.0, 33.0, 23.460072, 34.214043, 29.0, 24.000002, 16.0, 134.534576, 129.702774,
    4.831802, 108.287476, 103.765717, 98.935883, 86.917046, 78.47464, 72.0, 70.209007, 58.513847,
    43.113144, 31.430161, 7.311057, 26.591991, 16.215576, 32.39521, 33.976044, 39.0, 19.547207,
    7.831924, 13.212644, 33.75, 40.0, 12.39, 21.0, 22.0, 31.0, 80.132614, 52.953388, 72.5, 70.0,
    30.039848, 39.955551, 26.936478, 22.362938, 13.485558, 69.870239, 82.457291, 152.32753,
];
const HUMAN_NACHBAU: [f64; 58] = [
    159.999392, 0.0, 92.703599, 31.671013, 37.002184, 75.99754, 74.021366, 86.190036, 89.979591,
    53.997247, 45.882541, 35.99998, 33.000603, 23.45834, 34.231948, 29.001252, 24.000663,
    15.999677, 134.534602, 129.702515, 4.832087, 108.287288, 103.765666, 98.935884, 86.917146,
    78.474571, 71.999392, 70.209102, 58.514071, 43.113362, 31.430327, 7.311106, 26.591935,
    16.215651, 32.394733, 33.976625, 39.00007, 19.547186, 7.832194, 13.212839, 33.74432, 39.997615,
    12.379045, 21.039546, 21.999913, 30.981061, 80.132966, 52.953382, 72.500569, 69.99544,
    30.040067, 39.955373, 26.936491, 22.363086, 13.485321, 69.870259, 82.457176, 152.327435,
];
const HUMAN_BASE_E: [f64; 2] = [0.257258, 0.006749];
const HUMAN_BASE_GEMESSEN: [f64; 58] = [
    159.998988, 0.0, 92.675735, 32.069277, 36.794043, 79.774798, 74.731447, 91.439812, 99.208102,
    58.875932, 48.518127, 35.310266, 34.809291, 22.172315, 33.44168, 28.500309, 22.719541,
    14.587029, 136.109098, 132.100402, 4.008695, 116.443457, 110.300204, 103.298283, 91.033154,
    79.383072, 74.913207, 72.431395, 60.014816, 43.949842, 31.944581, 7.441525, 27.718634,
    18.669263, 32.893206, 32.147543, 37.324395, 18.604426, 7.600981, 11.174998, 31.737991,
    35.32864, 12.393839, 24.466374, 17.573897, 25.131134, 68.067574, 54.070713, 72.675139,
    73.753289, 31.759457, 41.993832, 28.385076, 21.369206, 14.898391, 70.191455, 80.215665,
    150.40712,
];
const HUMAN_DETAIL_E: [f64; 7] = [
    14.699706, 5.817769, 2.603545, 1.271556, 0.615153, 0.331247, 0.179595,
];
const HUMAN_F_DETAIL_START: [f64; 58] = [
    160.0, 0.0, 9.626526, 5.662974, 6.082763, 8.717798, 8.602325, 9.284395, 9.486833, 7.348469,
    6.965495, 6.0, 5.744563, 4.708749, 5.782878, 5.385165, 4.89898, 4.0, 136.109098, 132.100402,
    4.008695, 116.443457, 110.300204, 103.298283, 91.033154, 79.383072, 72.0, 72.431395, 60.014816,
    43.949842, 31.944581, 7.441525, 27.718634, 18.669263, 32.893206, 32.147543, 39.0, 18.604426,
    7.600981, 11.174998, 33.75, 40.0, 12.39, 21.0, 22.0, 31.0, 68.067574, 54.070713, 72.5, 70.0,
    31.759457, 41.993832, 28.385076, 21.369206, 14.898391, 70.191455, 80.215665, 150.40712,
];
const HUMAN_CLO: [f64; 58] = [
    160.0, 0.0, 92.669998, 31.671263, 37.000004, 76.000008, 74.0, 86.199997, 89.999992, 54.0,
    45.88224, 36.0, 33.0, 23.460072, 34.214043, 29.0, 24.000002, 16.0, 134.534576, 129.702774,
    4.831802, 108.287476, 103.765717, 98.935883, 86.917046, 78.47464, 72.0, 70.209007, 58.513847,
    43.113144, 31.430161, 7.311057, 26.591991, 16.215576, 32.39521, 33.976044, 39.0, 19.547207,
    7.831924, 13.212644, 33.75, 40.0, 12.39, 21.0, 22.0, 31.0, 80.132614, 52.953388, 72.5, 70.0,
    30.039848, 39.955551, 26.936478, 22.362938, 13.485558, 69.870239, 82.457291, 152.32753,
];
const SMIS_EINGABE: [f64; 58] = [
    160.0, 0.0, 92.67, 33.0, 37.0, 76.0, 74.0, 86.23477, 90.0, 54.0, 48.0, 36.0, 33.0, 22.0, 0.0,
    29.0, 24.0, 16.0, 135.0, 130.17, 5.0, 111.0, 0.0, 98.0, 0.0, 77.0, 72.0, 0.0, 0.0, 42.0, 30.0,
    8.0, 0.0, 0.0, 28.0, 0.0, 39.0, 0.0, 0.0, 0.0, 33.75, 40.0, 0.0, 21.0, 22.0, 31.0, 0.0, 53.0,
    0.0, 70.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];
const SMIS_NACHBAU: [f64; 58] = [
    159.999269, 0.0, 92.703269, 31.665538, 37.002293, 75.997669, 74.021726, 86.225158, 89.979821,
    53.997921, 45.887793, 36.000359, 33.00078, 23.464619, 34.200528, 29.001688, 24.001539,
    16.009253, 134.528237, 129.678433, 4.849803, 108.286043, 103.762784, 98.937033, 86.915467,
    78.478331, 71.999269, 70.2099, 58.518512, 43.122401, 31.436517, 7.314449, 26.582623, 16.216354,
    32.413347, 33.946295, 39.000134, 19.551729, 7.87179, 13.24448, 33.744109, 39.997063, 12.38343,
    21.039811, 22.000113, 30.980545, 80.126414, 53.125328, 72.677057, 69.995978, 30.048851,
    39.947127, 26.937764, 22.362378, 13.480757, 69.877832, 82.441494, 152.319326,
];
const SMIS_BASE_E: [f64; 2] = [0.256233, 0.005679];
const SMIS_DETAIL_E: [f64; 7] = [
    14.750585, 5.801575, 2.618128, 1.274822, 0.628218, 0.339869, 0.191294,
];
const SMIS_F_DETAIL_START: [f64; 58] = [
    160.0, 0.0, 9.626526, 5.662953, 6.082763, 8.717798, 8.602325, 9.286268, 9.486833, 7.348469,
    6.96547, 6.0, 5.744563, 4.708739, 5.78287, 5.385165, 4.898979, 4.0, 136.109079, 132.100374,
    4.008705, 116.443455, 110.300229, 103.298249, 91.0332, 79.383094, 72.0, 72.43145, 60.014841,
    43.949849, 31.944587, 7.441527, 27.718326, 18.668926, 32.893047, 32.147348, 39.0, 18.604352,
    7.600972, 11.17498, 33.75, 40.0, 12.39376, 21.0, 22.0, 31.0, 68.067164, 54.070672, 72.675024,
    70.0, 31.759255, 41.993579, 28.384972, 21.368896, 14.898436, 70.191156, 80.215334, 150.40649,
];
const GUI_B_EINGABE: [f64; 58] = [
    160.0, 0.0, 92.67, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0,
];
const GUI_B_NACHBAU: [f64; 58] = [
    159.998989, 0.0, 92.674667, 32.06904, 36.79379, 79.773704, 74.730294, 91.438641, 99.207399,
    58.875526, 48.517771, 35.310064, 34.80909, 22.172226, 33.441589, 28.499998, 22.719379,
    14.582751, 136.109079, 132.100374, 4.008705, 116.443455, 110.300229, 103.298249, 91.0332,
    79.383094, 74.913277, 72.43145, 60.014841, 43.949849, 31.944587, 7.441527, 27.718326,
    18.668926, 32.893047, 32.147348, 37.324229, 18.604352, 7.600972, 11.17498, 31.737949,
    35.328627, 12.39376, 24.466302, 17.573754, 25.131006, 68.067164, 54.070672, 72.675024,
    73.752834, 31.759255, 41.993579, 28.384972, 21.368896, 14.898436, 70.191156, 80.215334,
    150.40649,
];
const GUI_B_BASE_E: [f64; 2] = [0.256233, 0.005679];
const HUMAN_PUNKTE: [(usize, [f64; 3]); 30] = [
    (1949, [-0.233064, 159.999392, -1.045956]),
    (4246, [-0.233064, 130.824789, -1.868275]),
    (4324, [-0.233064, 136.711186, -10.87074]),
    (3960, [5.715396, 134.534602, -6.414631]),
    (19684, [-6.181523, 134.534602, -6.414631]),
    (7276, [17.352704, 129.702515, -7.021786]),
    (7299, [15.714895, 123.548003, -1.917881]),
    (9875, [16.019602, 124.335318, -12.734706]),
    (5407, [15.514143, 119.063804, -5.134761]),
    (5969, [10.342463, 110.362347, 8.117762]),
    (20335, [-9.509956, 103.765666, 5.040385]),
    (15535, [-0.233064, 97.670279, 8.577776]),
    (15749, [-0.233064, 86.055447, 9.622509]),
    (4978, [-0.233064, 79.588432, -14.785721]),
    (10011, [-0.233064, 71.999392, 2.30529]),
    (7727, [-0.043223, 70.209102, -2.448209]),
    (8850, [14.513232, 58.514071, -1.349318]),
    (11518, [13.112839, 43.113362, -5.758902]),
    (8038, [14.313484, 30.009536, -6.357392]),
    (10614, [5.409616, 7.311106, -4.17053]),
    (7099, [18.799837, 118.249566, -2.014007]),
    (12010, [25.63042, 101.986073, -8.192444]),
    (9601, [36.812441, 83.333331, -3.806956]),
    (4328, [-0.233064, 134.76257, -12.34129]),
    (12692, [43.028583, 68.796594, 2.097132]),
    (28311, [-43.49471, 68.796594, 2.097132]),
    (24765, [-36.113412, 83.136105, -0.647259]),
    (26396, [-8.659958, 0.000659, -6.108151]),
    (4654, [8.218904, 104.19614, 6.269873]),
    (10750, [6.392125, 2.88059, -3.052698]),
];

fn vergleiche(was: &str, ist: &[f64; ANZAHL], soll: &[f64; ANZAHL], tol: f64) {
    let groesste = (0..ANZAHL)
        .map(|j| (ist[j] - soll[j]).abs())
        .fold(0.0, f64::max);
    eprintln!("{was}: größte Abweichung {:.4} mm", groesste * 10.0);
    let mut fehler = Vec::new();
    for j in 0..ANZAHL {
        let d = ist[j] - soll[j];
        if d.abs() > tol {
            fehler.push(format!("{j:2} {:28} {:+.5} cm", NAMEN[j], d));
        }
    }
    assert!(fehler.is_empty(), "{was}:\n{}", fehler.join("\n"));
}

/// Startvektor der DETAIL-Stufe auf ihrer Maske (die übrigen Plätze wirken nicht).
fn start(was: &str, ist: &[f64; ANZAHL], soll: &[f64; ANZAHL], maske: &[usize]) {
    for &j in maske {
        assert!(
            (ist[j] - soll[j]).abs() <= ERSTE,
            "{was}: {j} {} {} statt {}",
            NAMEN[j],
            ist[j],
            soll[j]
        );
    }
}

fn stufe(was: &str, k: &Kalibrierung, e: &[f64], ende: Ende) {
    eprintln!(
        "{was}: E = {:?}, {:?}, {} Formungen",
        k.e_folge, k.ende, k.formungen
    );
    assert!(
        (k.e_folge[0] - e[0]).abs() <= ERSTE_E,
        "{was}: erster E-Wert {} statt {}",
        k.e_folge[0],
        e[0]
    );
    assert_eq!(k.ende, ende, "{was}: Ende-Grund");
    assert!(
        k.formungen.abs_diff(e.len()) <= 1,
        "{was}: {} Formungen statt {}",
        k.formungen,
        e.len()
    );
}

#[test]
fn basemesh_mit_f0_geformt() {
    let Some(o) = ordner() else { return };
    let k = Kette::laden(&o).expect("Daten");
    let mut f = [0.0; ANZAHL];
    f[0] = F0[0];
    f[2] = F0[2];
    let p = k.formen(&k.base, &f);
    let m = klon3d_koerper::messung::messen(&p, &k.messdaten).expect("Messung");
    // Feste Formung, aber Ebenen durch drei Punkte kippen am letzten Bit.
    vergleiche("BASE mit f0", &m.werte, &F0_WERTE, KETTE);
    let genau: f64 = [0, 2, 5, 6, 8]
        .iter()
        .map(|&j| (m.werte[j] - F0_WERTE[j]).abs())
        .fold(0.0, f64::max);
    assert!(genau <= 0.002, "Höhe/Umfänge {genau}");
    let s = [
        [-0.189757, 163.783437, -1.733989],
        [-0.189757, 136.52364, -1.068145],
        [-0.189757, 104.950777, 9.564422],
    ];
    for (n, &i) in [1949usize, 4246, 15535].iter().enumerate() {
        for c in 0..3 {
            assert!(
                (p[i][c] - s[n][c]).abs() <= 1.5e-4,
                "Punkt {i}: {:?} statt {:?}",
                p[i],
                s[n]
            );
        }
    }
}

#[test]
fn gui_fall_b_nur_base() {
    let Some(o) = ordner() else { return };
    let k = Kette::laden(&o).expect("Daten");
    let mut f = [0.0; ANZAHL];
    for &j in &k.base.maske {
        f[j] = klon3d_koerper::kette::g(j, GUI_B_EINGABE[j]);
    }
    let b = k.kalibrieren(&k.base, &f).expect("BASE");
    stufe("BASE", &b, &GUI_B_BASE_E, Ende::Getroffen);
    vergleiche("GUI-Fall B", &b.messung.werte, &GUI_B_NACHBAU, KETTE);
}

#[test]
fn emma_avs() {
    let Some(o) = ordner() else { return };
    let k = Kette::laden_mit(
        &o,
        "_BaseFeature_Bust_TotalHeight.txt",
        "_DetailFeature_Basic_Bust_TotalHeight.txt",
    )
    .expect("Daten");
    let avs = PathBuf::from(
        r"C:\Users\Public\Documents\CLO\Assets\Avatar\Avatar\Female_V1\Size\CLO_Female_Tall_0_Default.avs",
    );
    let v = if avs.is_file() {
        let (kopf, v) = lies_avs(&avs).expect("avs");
        assert_eq!(
            kopf,
            AvsKopf {
                anker: 0,
                detail: 0,
                breite: 0
            }
        );
        vergleiche("Emma .avs gelesen", &v, &EMMA_EINGABE, 1e-6);
        v
    } else {
        EMMA_EINGABE
    };
    let e = k.aus_massen(&v).expect("Kette");
    stufe("BASE", &e.base, &EMMA_BASE_E, Ende::Getroffen);
    vergleiche(
        "Emma BASE",
        &e.base.messung.werte,
        &EMMA_BASE_GEMESSEN,
        KETTE,
    );
    start(
        "Emma DETAIL-Start",
        &e.f_detail_start,
        &EMMA_F_DETAIL_START,
        &k.detail.maske,
    );
    stufe("DETAIL", &e.detail, &EMMA_DETAIL_E, Ende::Getroffen);
    vergleiche("Emma Endwerte", e.werte(), &EMMA_NACHBAU, KETTE);
    vergleiche_clo("Emma gegen CLO", e.werte(), &EMMA_CLO, &k.detail.maske);
    assert!((e.hand_t - 0.160271).abs() < 1e-5, "hand_t {}", e.hand_t);
}

/// Gegen CLOs gespeicherte Werte: getriebene Maße ≤ 0,6 mm, übrige ≤ 0,3 mm.
fn vergleiche_clo(was: &str, ist: &[f64; ANZAHL], clo: &[f64; ANZAHL], maske: &[usize]) {
    let (mut getrieben, mut frei) = (0.0f64, 0.0f64);
    for j in 0..ANZAHL {
        let d = (ist[j] - clo[j]).abs() * 10.0;
        if maske.contains(&j) {
            getrieben = getrieben.max(d);
        } else {
            frei = frei.max(d);
        }
    }
    eprintln!("{was}: getrieben {getrieben:.4} mm, frei {frei:.4} mm");
    let mut fehler = Vec::new();
    for j in 0..ANZAHL {
        let tol = if maske.contains(&j) { 0.06 } else { 0.03 };
        let d = ist[j] - clo[j];
        if d.abs() > tol {
            fehler.push(format!("{j:2} {:28} {:+.5} cm", NAMEN[j], d));
        }
    }
    assert!(fehler.is_empty(), "{was}:\n{}", fehler.join("\n"));
}

#[test]
fn anwenderin_human_avs() {
    let Some(o) = ordner() else { return };
    let k = Kette::laden(&o).expect("Daten");
    let avs = PathBuf::from(r"D:\ai\rea\work\reinraum\clo\daten\export\anwenderin_human.avs");
    let v = if avs.is_file() {
        let (kopf, v) = lies_avs(&avs).expect("avs");
        assert_eq!(
            kopf,
            AvsKopf {
                anker: 0,
                detail: 1,
                breite: 0
            }
        );
        v
    } else {
        // Werte der .avs sind float32; die Prüfwerte runden auf 6 Stellen.
        HUMAN_EINGABE.map(|x| x as f32 as f64)
    };
    let e = k.aus_massen(&v).expect("Kette");
    stufe("BASE", &e.base, &HUMAN_BASE_E, Ende::Getroffen);
    vergleiche("BASE", &e.base.messung.werte, &HUMAN_BASE_GEMESSEN, KETTE);
    start(
        "DETAIL-Start",
        &e.f_detail_start,
        &HUMAN_F_DETAIL_START,
        &k.detail.maske,
    );
    stufe("DETAIL", &e.detail, &HUMAN_DETAIL_E, Ende::Getroffen);
    vergleiche("Endwerte", e.werte(), &HUMAN_NACHBAU, KETTE);
    vergleiche_clo("gegen CLO", e.werte(), &HUMAN_CLO, &k.detail.maske);
    assert!((e.hand_t - 0.090906).abs() < 1e-5, "hand_t {}", e.hand_t);
    let mut weit = Vec::new();
    let mut groesste = 0.0f64;
    for (i, s) in HUMAN_PUNKTE {
        let p = e.detail.punkte[i];
        let d = ((p[0] - s[0]).powi(2) + (p[1] - s[1]).powi(2) + (p[2] - s[2]).powi(2)).sqrt();
        groesste = groesste.max(d);
        if d > PUNKT {
            weit.push(format!("{i}: {d:.5} cm"));
        }
    }
    eprintln!("Punkte: größte Abweichung {:.4} mm", groesste * 10.0);
    assert!(weit.is_empty(), "Punkte:\n{}", weit.join("\n"));
}

#[test]
fn anwenderin_smis_eingabe() {
    let Some(o) = ordner() else { return };
    let k = Kette::laden(&o).expect("Daten");
    let e = k.aus_massen(&SMIS_EINGABE).expect("Kette");
    stufe("BASE", &e.base, &SMIS_BASE_E, Ende::Getroffen);
    start(
        "DETAIL-Start",
        &e.f_detail_start,
        &SMIS_F_DETAIL_START,
        &k.detail.maske,
    );
    stufe("DETAIL", &e.detail, &SMIS_DETAIL_E, Ende::Getroffen);
    vergleiche("Endwerte", e.werte(), &SMIS_NACHBAU, KETTE);
}

/// Dieselbe Kette, aber aus der `.smis` über die Zuordnung von
/// `massdefinitionen.md`; HighHip leitet die Kette nach der BASE-Stufe ab.
#[test]
fn anwenderin_aus_smis() {
    let Some(o) = ordner() else { return };
    let pfad = std::env::var_os("KLON3D_SMIS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../klon3d-pattern/tests/muster/measurements.smis"
            ))
        });
    let Ok(smis) = klon3d_pattern::lies_smis(&pfad) else {
        eprintln!("übersprungen: {} fehlt", pfad.display());
        return;
    };
    let k = Kette::laden(&o).expect("Daten");
    let e = k.aus_smis(&|n| smis.wert(n)).expect("Kette");
    // HighHip hängt am BASE-Netz (d = 12,39376 in den Prüfwerten).
    vergleiche("Vorgaben", &e.eingabe, &SMIS_EINGABE, 1e-4);
    stufe("DETAIL", &e.detail, &SMIS_DETAIL_E, Ende::Getroffen);
    vergleiche("Endwerte", e.werte(), &SMIS_NACHBAU, KETTE);
}

/// Abnahme der Stufe 2 am Körper aus der `.smis`: Umfänge ±1 cm der Maße
/// (PLAN.md), gemessen wie CLO am Kandidaten und als Abtastung
/// (`massdefinitionen.md`). HighHip gilt an CLOs Lage, also gegen den
/// abgeleiteten Wert, nicht gegen `highhip_circ` am Beckenkamm.
#[test]
fn abnahme_aus_smis() {
    let Some(o) = ordner() else { return };
    let pfad = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../klon3d-pattern/tests/muster/measurements.smis"
    ));
    let Ok(smis) = klon3d_pattern::lies_smis(&pfad) else {
        eprintln!("übersprungen: {} fehlt", pfad.display());
        return;
    };
    let k = Kette::laden(&o).expect("Daten");
    let e = k.aus_smis(&|n| smis.wert(n)).expect("Kette");
    let m = e.werte();
    let soll = |n: &str| smis.wert(n).unwrap();
    let mut fehler = Vec::new();
    for (was, ist, ziel) in [
        ("Brust", m[2], soll("bust_circ")),
        ("Unterbrust", m[5], soll("lowbust_circ")),
        ("Taille", m[6], soll("waist_circ")),
        ("HighHip an CLOs Lage", m[7], e.eingabe[7]),
        ("Hüfte", m[8], soll("hip_circ")),
    ] {
        eprintln!("{was}: {ist:.3} (Soll {ziel:.3}, {:+.3})", ist - ziel);
        if (ist - ziel).abs() > 1.0 {
            fehler.push(was);
        }
    }

    let r = rumpfmasse(&e.detail.punkte, &k.messdaten).expect("Rumpfmaße");
    let a = abnahme(&e.detail.punkte, &k.messdaten, &r).expect("Abnahme");
    let (t, h) = (a.taille.expect("Taille"), a.huefte.expect("Hüfte"));
    eprintln!(
        "Abtastung: Taille {:.4} bei y {:.3}, Hüfte {:.4} bei y {:.3}",
        t.umfang, t.y, h.umfang, h.y
    );
    // Prüfwerte `anwenderin_smis.abnahme` (Raster 0,25 cm ab anderer Höhe).
    assert!(
        (t.umfang - 73.1095).abs() < 0.03 && (t.y - 101.228).abs() <= 0.25,
        "Taille {t:?}"
    );
    assert!(
        (h.umfang - 89.9864).abs() < 0.03 && (h.y - 78.749).abs() <= 0.25,
        "Hüfte {h:?}"
    );
    for (was, ist, ziel) in [
        ("Taille abgetastet", t.umfang, soll("waist_circ")),
        ("Hüfte abgetastet", h.umfang, soll("hip_circ")),
    ] {
        if (ist - ziel).abs() > 1.0 {
            fehler.push(was);
        }
    }
    assert!(fehler.is_empty(), "außerhalb ±1 cm: {fehler:?}");
}

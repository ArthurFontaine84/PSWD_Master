// Pas de fenêtre de console derrière l'application sous Windows (en version finale).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

// Exclu de la couverture : point d'entrée du processus sur le bureau ; l'application est testée
// par ses commandes.
#[cfg_attr(coverage_nightly, coverage(off))]
fn main() {
    pswd_gui_lib::run();
}

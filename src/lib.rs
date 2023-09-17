use flate2::read::GzDecoder;
use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use std::fs::File;
use std::io::{self, prelude::*};
use std::path::PathBuf;

use hashbrown::HashSet;
use log::{debug, info, trace};

use pyo3::prelude::*;

use symspell::DistanceAlgorithm;
use symspell::{SymSpell, SymSpellBuilder, UnicodeStringStrategy, Verbosity};

fn read_barcodes(barcode_file: PathBuf) -> io::Result<Vec<String>> {
    let file = File::open(barcode_file)?;
    let reader = io::BufReader::new(GzDecoder::new(file));

    Ok(reader.lines().filter_map(|line| line.ok()).collect())
}

#[pyclass(frozen)]
struct BarcodeSet {
    symspell: SymSpell<UnicodeStringStrategy>,
    #[pyo3(get)]
    max_dist: i64,
    #[pyo3(get)]
    prefix_length: i64,
    #[pyo3(get)]
    barcode_length: usize,
}

#[pymethods]
impl BarcodeSet {
    /// construct a BarcodeSet, which is a set of barcodes stored in a symspell index
    /// for fast lookup and error correction
    #[new]
    fn py_new(barcode_file: PathBuf, max_dist: i64, prefix_length: i64) -> PyResult<Self> {
        let builder = SymSpellBuilder::default()
            .max_dictionary_edit_distance(max_dist)
            .prefix_length(prefix_length)
            .distance_algorithm(DistanceAlgorithm::Levenshtein)
            .build();

        if let Ok(mut symspell) = builder {
            info!("Reading barcodes from {}", barcode_file.display());
            if let Ok(barcodes) = read_barcodes(barcode_file) {
                info!("Loading barcodes");
                for bc in barcodes.iter() {
                    symspell.create_dictionary_entry(bc);
                }

                let barcode_length: HashSet<_> = barcodes.iter().map(|bc| bc.len()).collect();
                if barcode_length.len() == 1 {
                    debug!("Built SymSpell index with {} barcodes", barcodes.len());
                    let barcode_length = barcode_length.iter().nth(0).unwrap().clone();
                    Ok(BarcodeSet {
                        symspell,
                        max_dist,
                        prefix_length,
                        barcode_length,
                    })
                } else {
                    Err(PyValueError::new_err(
                        "Found barcodes with multiple lengths",
                    ))
                }
            } else {
                Err(PyIOError::new_err("Error reading barcode file"))
            }
        } else {
            return Err(PyRuntimeError::new_err("Error building symspell"));
        }
    }

    /// Looks up a single word and returns all the closest suggestions (i.e. all words
    /// in the collection at the best distance), or an empty list if none are found.
    fn lookup(&self, query: &str) -> PyResult<Vec<(String, String, i64)>> {
        trace!("Searching for {}", query);
        let suggestions = self
            .symspell
            .lookup(query, Verbosity::Closest, self.max_dist);

        Ok(suggestions
            .iter()
            .cloned()
            .map(|s| (s.term, s.query, s.distance))
            .collect())
    }

    /// Looks up a batch of related strings to see if together they match to a single
    /// word. Returns all matches at the minimum distance, or an empty list.
    fn lookup_batch(&self, queries: HashSet<&str>) -> PyResult<Vec<(String, String, i64)>> {
        trace!("Searching for {} queries", queries.len());

        let suggestions = self.symspell.exact_lookup_batch(&queries);
        if suggestions.len() > 0 {
            return Ok(suggestions
                .iter()
                .map(|s| (s.term.clone(), s.term.clone(), s.distance))
                .collect());
        }

        let suggestions: Vec<_> = queries
            .iter()
            .flat_map(|q| self.symspell.lookup(q, Verbosity::Closest, self.max_dist))
            .collect();

        if suggestions.len() == 0 {
            return Ok(Vec::new());
        }

        let min_dist = suggestions.iter().map(|s| s.distance).min().unwrap();
        let suggestions: HashSet<_> = suggestions
            .iter()
            .filter(|s| s.distance == min_dist)
            .cloned()
            .map(|s| (s.term, s.query, s.distance))
            .collect();

        Ok(suggestions.iter().cloned().collect())
    }

    /// Takes a string and look up all substrings that might plausibly be in the barcode
    /// set. This is based on max edit distance and barcode length
    fn lookup_substrings(&self, query: &str) -> PyResult<Vec<(String, String, i64)>> {
        let max_dist = self.max_dist as usize;
        if query.len() < (self.barcode_length - max_dist) {
            return Ok(Vec::new());
        }
        let mut queries = HashSet::new();

        for i in 0..(query.len() - self.barcode_length + 2 * max_dist) {
            for j in 0..(2 * max_dist + 1) {
                let k = i + j + self.barcode_length - max_dist;
                if k <= query.len() {
                    queries.insert(&query[i..k]);
                }
            }
        }

        self.lookup_batch(queries)
    }
}

#[pyfunction]
fn check_symspell(barcode_file: PathBuf, q: &str, max_dist: i64) -> PyResult<Vec<(String, i64)>> {
    let mut symspell: SymSpell<UnicodeStringStrategy> = SymSpellBuilder::default()
        .max_dictionary_edit_distance(max_dist)
        .prefix_length(16)
        .distance_algorithm(DistanceAlgorithm::Levenshtein)
        .build()
        .unwrap();

    debug!("Reading the barcode file");
    let barcodes = read_barcodes(barcode_file)?;

    debug!("Loading barcodes");
    for bc in barcodes.iter() {
        symspell.create_dictionary_entry(bc);
    }
    // symspell.load_dictionary(barcode_file, 0, 1, "\t");

    debug!("Searching for {}", q);
    let suggestions = symspell.lookup(q, Verbosity::Closest, max_dist);

    debug!("Done, found {} suggestions", suggestions.len());
    Ok(suggestions
        .iter()
        .map(|s| (s.term.clone(), s.distance))
        .collect())
}

/// Takes a query and a whitelist of barcodes. Builds a DFA for each barcode
/// in the whitelist and checks the query
// #[pyfunction]
// fn check_query(barcode_file: PathBuf, q: &str, max_dist: u8) -> PyResult<Vec<u8>> {
//     debug!("Reading the barcodes");
//     let barcodes = read_barcodes(barcode_file)?;

//     debug!("Building all the automata");
//     let automata = build_automata(&barcodes, max_dist);

//     debug!("Searching for {}", q);
//     let res = automata
//         .iter()
//         .map(|a| match a.eval(q) {
//             Distance::Exact(i) => i,
//             Distance::AtLeast(i) => i,
//         })
//         .collect();
//     debug!("Done");

//     Ok(res)
// }

/// A Python module implemented in Rust.
#[pymodule]
fn barcode_automata(_py: Python, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();

    // m.add_function(wrap_pyfunction!(check_rocks, m)?)?;
    m.add_class::<BarcodeSet>()?;
    m.add_function(wrap_pyfunction!(check_symspell, m)?)?;
    // m.add_function(wrap_pyfunction!(check_query, m)?)?;
    Ok(())
}

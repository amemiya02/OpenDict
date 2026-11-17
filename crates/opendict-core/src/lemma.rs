//! Lemmatization: the base forms an inflected or conjugated word may come from.
//!
//! The rules here only propose base forms. Lookup keeps a proposal only when a
//! Dictionary in the Library files it, so a rule may be generous: "runn" and "runne"
//! are proposed for "running" next to "run", and are simply never found.
//!
//! The rule tables and word lists in this module are OpenDict's own work, written for
//! this project (GPL-3.0-or-later, like the rest of `opendict-core`).

/// A base form proposed for a word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseForm {
    pub form: String,
    /// The ending is just as common in base forms as in inflected ones ("number",
    /// "after", "かわいそう"), so the proposal is worth making only when the word itself
    /// is not a headword.
    pub doubtful: bool,
}

/// The base forms `word` may be an inflected (English) or conjugated (Japanese) form
/// of, most likely first. `word` is expected as an index key (see
/// [`index_key`](crate::index_key)): English in lower case. The word itself is never
/// among them.
pub fn base_forms(word: &str) -> Vec<BaseForm> {
    let mut forms: Vec<BaseForm> = Vec::new();
    let mut add = |form: String, doubtful: bool| {
        if !form.is_empty() && form != word && !forms.iter().any(|known| known.form == form) {
            forms.push(BaseForm { form, doubtful });
        }
    };
    for (form, doubtful) in english::base_forms(word) {
        add(form, doubtful);
    }
    forms
}

mod english {
    /// Base forms for an English word, each with whether it is doubtful.
    pub(super) fn base_forms(word: &str) -> Vec<(String, bool)> {
        let mut forms = Vec::new();
        if !word.chars().all(|c| c.is_ascii_lowercase() || c == '\'' || c == '’') {
            return forms;
        }
        let sure = |forms: &mut Vec<(String, bool)>, form: String| forms.push((form, false));

        for (inflected, bases) in IRREGULAR {
            if *inflected == word {
                for base in bases.split(' ') {
                    sure(&mut forms, base.to_owned());
                }
            }
        }

        // Possessives: "reader's", "readers'".
        for ending in ["'s", "’s"] {
            if let Some(stem) = word.strip_suffix(ending) {
                sure(&mut forms, stem.to_owned());
            }
        }
        for ending in ["'", "’"] {
            if let Some(stem) = word.strip_suffix(ending).filter(|stem| stem.ends_with('s')) {
                sure(&mut forms, stem.to_owned());
            }
        }

        // Plurals and the third person: the plain "-s" first, so "gives" finds "give"
        // before "gif".
        if let Some(stem) = word.strip_suffix('s').filter(|stem| long_enough(stem)) {
            // "his" and "yes" are no plurals: the plain stem needs three letters.
            if stem.len() >= 3 && !stem.ends_with('s') && !stem.ends_with('u') {
                sure(&mut forms, stem.to_owned());
            }
            if let Some(stem) = stem.strip_suffix('e') {
                sure(&mut forms, stem.to_owned());
                if let Some(stem) = stem.strip_suffix('i') {
                    sure(&mut forms, format!("{stem}y"));
                }
                if let Some(stem) = stem.strip_suffix('v') {
                    sure(&mut forms, format!("{stem}f"));
                    sure(&mut forms, format!("{stem}fe"));
                }
            }
        }

        // Past tense and participles.
        if let Some(stem) = word.strip_suffix("ed").filter(|stem| long_enough(stem)) {
            if let Some(stem) = stem.strip_suffix('i') {
                sure(&mut forms, format!("{stem}y"));
            }
            for form in verb_stem(stem) {
                sure(&mut forms, form);
            }
        }
        if let Some(stem) = word.strip_suffix("ing").filter(|stem| long_enough(stem)) {
            if let Some(stem) = stem.strip_suffix('y') {
                sure(&mut forms, format!("{stem}ie"));
            }
            for form in verb_stem(stem) {
                sure(&mut forms, form);
            }
        }

        // Comparison. "-er" and "-est" end plenty of base forms (number, after, forest),
        // so these are doubtful.
        for ending in ["er", "est"] {
            if let Some(stem) = word.strip_suffix(ending).filter(|stem| long_enough(stem)) {
                if let Some(stem) = stem.strip_suffix('i') {
                    forms.push((format!("{stem}y"), true));
                }
                for form in verb_stem(stem) {
                    forms.push((form, true));
                }
            }
        }
        forms
    }

    /// A stem needs a vowel to be a word: "s" from "sing" is not one.
    fn long_enough(stem: &str) -> bool {
        stem.chars().count() >= 2 && stem.chars().any(is_vowel)
    }

    fn is_vowel(c: char) -> bool {
        matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
    }

    /// What a stem left by "-ed", "-ing", "-er" or "-est" may come from: undoubled
    /// ("runn" → "run"), with its silent "e" back ("mak" → "make"), or as it is
    /// ("walk"). The order follows what the stem looks like: after one short vowel and a
    /// consonant ("hop", "cod") the "e" usually went missing, so "hope" comes before
    /// "hop"; otherwise ("sing", "eat") the stem as it is comes first.
    fn verb_stem(stem: &str) -> Vec<String> {
        let chars: Vec<char> = stem.chars().collect();
        let mut forms = Vec::new();
        let n = chars.len();
        if n >= 3
            && chars[n - 1] == chars[n - 2]
            && !is_vowel(chars[n - 1])
            && !matches!(chars[n - 1], 'l' | 's' | 'f' | 'z')
        {
            forms.push(chars[..n - 1].iter().collect());
        }
        let last = chars[n - 1];
        let takes_e = !matches!(last, 'a' | 'o' | 'w' | 'x' | 'y');
        let with_e = format!("{stem}e");
        let short_vowel_consonant = n >= 2
            && !is_vowel(last)
            && is_vowel(chars[n - 2])
            && (n == 2 || !is_vowel(chars[n - 3]));
        if takes_e && (short_vowel_consonant || matches!(last, 'e' | 'i' | 'u')) {
            forms.push(with_e);
            forms.push(stem.to_owned());
        } else {
            forms.push(stem.to_owned());
            if takes_e {
                forms.push(with_e);
            }
        }
        forms
    }

    /// Irregular forms and their base forms (space-separated, most likely first).
    #[rustfmt::skip]
    const IRREGULAR: &[(&str, &str)] = &[
        // be, have, do and the modal-like
        ("am", "be"), ("is", "be"), ("are", "be"), ("was", "be"), ("were", "be"),
        ("been", "be"), ("being", "be"), ("has", "have"), ("had", "have"),
        ("does", "do"), ("did", "do"), ("done", "do"),
        // strong and irregular verbs
        ("arose", "arise"), ("arisen", "arise"), ("awoke", "awake"), ("awoken", "awake"),
        ("bore", "bear"), ("borne", "bear"), ("born", "bear"),
        ("became", "become"), ("began", "begin"), ("begun", "begin"), ("bent", "bend"),
        ("bit", "bite"), ("bitten", "bite"), ("bled", "bleed"), ("blew", "blow"),
        ("blown", "blow"), ("broke", "break"), ("broken", "break"), ("bred", "breed"),
        ("brought", "bring"), ("built", "build"), ("burnt", "burn"), ("bought", "buy"),
        ("caught", "catch"), ("chose", "choose"), ("chosen", "choose"), ("clung", "cling"),
        ("came", "come"), ("crept", "creep"), ("dealt", "deal"), ("dug", "dig"),
        ("drew", "draw"), ("drawn", "draw"), ("dreamt", "dream"), ("drank", "drink"),
        ("drunk", "drink"), ("drove", "drive"), ("driven", "drive"), ("ate", "eat"),
        ("eaten", "eat"), ("fell", "fall"), ("fallen", "fall"), ("fed", "feed"),
        ("felt", "feel"), ("fought", "fight"), ("found", "find"), ("fled", "flee"),
        ("flung", "fling"), ("flew", "fly"), ("flown", "fly"), ("forbade", "forbid"),
        ("forbidden", "forbid"), ("forgot", "forget"), ("forgotten", "forget"),
        ("forgave", "forgive"), ("forgiven", "forgive"), ("froze", "freeze"),
        ("frozen", "freeze"), ("got", "get"), ("gotten", "get"), ("gave", "give"),
        ("given", "give"), ("went", "go"), ("gone", "go"), ("goes", "go"),
        ("ground", "grind"), ("grew", "grow"), ("grown", "grow"), ("hung", "hang"),
        ("heard", "hear"), ("hid", "hide"), ("hidden", "hide"), ("held", "hold"),
        ("kept", "keep"), ("knelt", "kneel"), ("knew", "know"), ("known", "know"),
        ("laid", "lay"), ("led", "lead"), ("leapt", "leap"), ("learnt", "learn"),
        ("left", "leave"), ("lent", "lend"), ("lay", "lie"), ("lain", "lie"),
        ("lit", "light"), ("lost", "lose"), ("made", "make"), ("meant", "mean"),
        ("met", "meet"), ("mistook", "mistake"), ("mistaken", "mistake"),
        ("overcame", "overcome"), ("paid", "pay"), ("ran", "run"), ("rang", "ring"),
        ("rung", "ring"), ("rode", "ride"), ("ridden", "ride"), ("rose", "rise"),
        ("risen", "rise"), ("said", "say"), ("saw", "see"), ("seen", "see"),
        ("sought", "seek"), ("sold", "sell"), ("sent", "send"), ("shook", "shake"),
        ("shaken", "shake"), ("shone", "shine"), ("shot", "shoot"), ("shrank", "shrink"),
        ("shrunk", "shrink"), ("sang", "sing"), ("sung", "sing"), ("sank", "sink"),
        ("sunk", "sink"), ("sat", "sit"), ("slew", "slay"), ("slain", "slay"),
        ("slept", "sleep"), ("slid", "slide"), ("slung", "sling"), ("smelt", "smell"),
        ("spoke", "speak"), ("spoken", "speak"), ("sped", "speed"), ("spelt", "spell"),
        ("spent", "spend"), ("spilt", "spill"), ("spun", "spin"), ("spat", "spit"),
        ("sprang", "spring"), ("sprung", "spring"), ("stood", "stand"), ("stole", "steal"),
        ("stolen", "steal"), ("stuck", "stick"), ("stung", "sting"), ("stank", "stink"),
        ("stunk", "stink"), ("strode", "stride"), ("struck", "strike"), ("strung", "string"),
        ("strove", "strive"), ("striven", "strive"), ("swore", "swear"), ("sworn", "swear"),
        ("swept", "sweep"), ("swam", "swim"), ("swum", "swim"), ("swung", "swing"),
        ("took", "take"), ("taken", "take"), ("taught", "teach"), ("tore", "tear"),
        ("torn", "tear"), ("told", "tell"), ("thought", "think"), ("threw", "throw"),
        ("thrown", "throw"), ("trod", "tread"), ("trodden", "tread"),
        ("understood", "understand"), ("woke", "wake"), ("woken", "wake"),
        ("wore", "wear"), ("worn", "wear"), ("wove", "weave"), ("woven", "weave"),
        ("wept", "weep"), ("won", "win"), ("wound", "wind"), ("wrung", "wring"),
        ("wrote", "write"), ("written", "write"),
        // nouns
        ("men", "man"), ("women", "woman"), ("children", "child"), ("people", "person"),
        ("mice", "mouse"), ("lice", "louse"), ("geese", "goose"), ("feet", "foot"),
        ("teeth", "tooth"), ("oxen", "ox"), ("dice", "die"), ("data", "datum"),
        ("media", "medium"), ("bacteria", "bacterium"), ("criteria", "criterion"),
        ("phenomena", "phenomenon"), ("analyses", "analysis"), ("bases", "basis base"),
        ("crises", "crisis"), ("theses", "thesis"), ("hypotheses", "hypothesis"),
        ("diagnoses", "diagnosis"), ("oases", "oasis"), ("cacti", "cactus"),
        ("fungi", "fungus"), ("nuclei", "nucleus"), ("radii", "radius"),
        ("stimuli", "stimulus"), ("alumni", "alumnus"), ("indices", "index"),
        ("matrices", "matrix"), ("appendices", "appendix"), ("vertices", "vertex"),
        // comparison
        ("better", "good well"), ("best", "good well"), ("worse", "bad badly"),
        ("worst", "bad badly"), ("more", "much many"), ("most", "much many"),
        ("less", "little"), ("least", "little"), ("further", "far"), ("furthest", "far"),
        ("farther", "far"), ("farthest", "far"), ("elder", "old"), ("eldest", "old"),
    ];
}

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
    let proposals = english::base_forms(word)
        .into_iter()
        .chain(japanese::base_forms(word));
    for (form, doubtful) in proposals {
        if form.is_empty() || form == word {
            continue;
        }
        match forms.iter_mut().find(|known| known.form == form) {
            Some(known) => known.doubtful &= doubtful,
            None => forms.push(BaseForm { form, doubtful }),
        }
    }
    forms
}

mod japanese {
    use std::sync::OnceLock;

    // What a word is, as far as the rules are concerned. A rule accepts some kinds of
    // word and leaves a word of one kind, so rules chain only where the grammar does:
    // 食べなかった → 食べない (an adjective-like form) → 食べる (an ichidan verb).
    /// The word as typed: every rule may start from it.
    const TYPED: u16 = 1;
    /// Ichidan verb (食べる), and the passive, causative and potential forms that
    /// conjugate like one.
    const V1: u16 = 1 << 1;
    /// Godan verb (書く).
    const V5: u16 = 1 << 2;
    /// 来る.
    const VK: u16 = 1 << 3;
    /// する and nouns taking する (勉強する).
    const VS: u16 = 1 << 4;
    /// I-adjective (儚い), and the ない and たい forms that conjugate like one.
    const ADJ: u16 = 1 << 5;
    /// A te-form (食べて): not a base form, but ている and てる lead back to it.
    const TE: u16 = 1 << 6;
    /// A noun that took する.
    const NOUN: u16 = 1 << 7;
    const ANY: u16 = u16::MAX;

    /// How many rules may chain: 食べさせられなかった needs four.
    const MAX_STEPS: usize = 4;

    struct Rule {
        from: String,
        to: String,
        accepts: u16,
        gives: u16,
        /// The ending is common in base forms too (届ける is not 届く's potential).
        doubtful: bool,
        /// The rule may consume the whole word (した → する).
        whole: bool,
    }

    /// Base forms for a Japanese word, each with whether it is doubtful, nearest first.
    pub(super) fn base_forms(word: &str) -> Vec<(String, bool)> {
        let mut found: Vec<(String, u16, bool)> = Vec::new();
        let mut frontier = vec![(word.to_owned(), ANY, false)];
        for _ in 0..MAX_STEPS {
            let mut next = Vec::new();
            for (form, kind, doubtful) in &frontier {
                for rule in rules() {
                    if kind & rule.accepts == 0 {
                        continue;
                    }
                    let Some(stem) = form.strip_suffix(rule.from.as_str()) else {
                        continue;
                    };
                    if stem.is_empty() && !rule.whole {
                        continue;
                    }
                    let base = format!("{stem}{}", rule.to);
                    let doubtful = *doubtful || rule.doubtful;
                    if base.is_empty()
                        || found
                            .iter()
                            .any(|(known, gives, _)| *known == base && *gives == rule.gives)
                    {
                        continue;
                    }
                    found.push((base.clone(), rule.gives, doubtful));
                    next.push((base, rule.gives, doubtful));
                }
            }
            frontier = next;
        }
        found
            .into_iter()
            .filter(|(_, kind, _)| *kind != TE)
            .map(|(form, _, doubtful)| (form, doubtful))
            .collect()
    }

    fn rules() -> &'static [Rule] {
        static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
        RULES.get_or_init(build)
    }

    /// Polite and "while" endings, after the stem each verb class puts before ます.
    const POLITE: &[&str] = &[
        "ます",
        "ました",
        "ません",
        "ませんでした",
        "ましょう",
        "ながら",
    ];

    #[rustfmt::skip]
    fn build() -> Vec<Rule> {
        let mut rules = Vec::new();
        let mut rule = |from: String, to: &str, accepts: u16, gives: u16, doubtful: bool| {
            let whole = matches!(to, "する" | "くる" | "来る" | "行く" | "いく");
            rules.push(Rule { from, to: to.to_owned(), accepts, gives, doubtful, whole });
        };

        // Ichidan verbs: 食べ + ending ← 食べる.
        for ending in ["ない", "たい"] { rule(ending.into(), "る", ADJ, V1, false); }
        for ending in ["ず", "ずに", "ぬ", "た", "たら", "たり", "よう", "れば"]
            .iter().chain(POLITE)
        {
            rule((*ending).into(), "る", TYPED, V1, false);
        }
        rule("て".into(), "る", TE, V1, false);
        for ending in ["ろ", "よ"] { rule(ending.into(), "る", TYPED, V1, true); }
        for ending in ["られる", "させる", "させられる"] { rule(ending.into(), "る", V1, V1, true); }

        // Godan verbs, by the row their dictionary form ends in:
        // (dictionary ending, a-, i-, e-, o-stem, te-form, ta-form).
        const ROWS: &[(&str, &str, &str, &str, &str, &str, &str)] = &[
            ("う", "わ", "い", "え", "お", "って", "った"),
            ("く", "か", "き", "け", "こ", "いて", "いた"),
            ("ぐ", "が", "ぎ", "げ", "ご", "いで", "いだ"),
            ("す", "さ", "し", "せ", "そ", "して", "した"),
            ("つ", "た", "ち", "て", "と", "って", "った"),
            ("ぬ", "な", "に", "ね", "の", "んで", "んだ"),
            ("ぶ", "ば", "び", "べ", "ぼ", "んで", "んだ"),
            ("む", "ま", "み", "め", "も", "んで", "んだ"),
            ("る", "ら", "り", "れ", "ろ", "って", "った"),
        ];
        for &(dict, a, i, e, o, te, ta) in ROWS {
            rule(format!("{a}ない"), dict, ADJ, V5, false);
            for ending in ["ず", "ずに", "ぬ"] { rule(format!("{a}{ending}"), dict, TYPED, V5, false); }
            for ending in ["れる", "せる", "せられる", "される"] {
                rule(format!("{a}{ending}"), dict, V1, V5, true);
            }
            for ending in POLITE { rule(format!("{i}{ending}"), dict, TYPED, V5, false); }
            rule(format!("{i}たい"), dict, ADJ, V5, false);
            rule(format!("{e}ば"), dict, TYPED, V5, false);
            rule(e.into(), dict, TYPED, V5, true);
            rule(format!("{e}る"), dict, V1, V5, true);
            rule(format!("{o}う"), dict, TYPED, V5, false);
            rule(te.into(), dict, TE, V5, false);
            for ending in ["", "ら", "り"] { rule(format!("{ta}{ending}"), dict, TYPED, V5, false); }
        }
        // 行く makes its te- and ta-forms with っ.
        for (go, base) in [("行", "行く"), ("い", "いく")] {
            rule(format!("{go}って"), base, TE, V5, false);
            for ending in ["", "ら", "り"] { rule(format!("{go}った{ending}"), base, TYPED, V5, false); }
        }

        // する, and nouns that take it.
        rule("しない".into(), "する", ADJ, VS, false);
        rule("したい".into(), "する", ADJ, VS, false);
        for ending in ["せず", "せずに", "した", "したら", "したり", "しよう", "しろ", "せよ", "すれば"] {
            rule(ending.into(), "する", TYPED, VS, false);
        }
        for ending in POLITE { rule(format!("し{ending}"), "する", TYPED, VS, false); }
        rule("して".into(), "する", TE, VS, false);
        for ending in ["される", "させる", "させられる"] { rule(ending.into(), "する", V1, VS, true); }
        rule("する".into(), "", VS, NOUN, false);

        // 来る, in kanji and in kana: (stem before ない, before ます, before ば, dictionary form).
        for (nai, masu, ba, base) in [("来", "来", "来", "来る"), ("こ", "き", "く", "くる")] {
            rule(format!("{nai}ない"), base, ADJ, VK, false);
            rule(format!("{masu}たい"), base, ADJ, VK, false);
            for ending in ["ず", "ずに", "よう", "い"] { rule(format!("{nai}{ending}"), base, TYPED, VK, false); }
            for ending in ["た", "たら", "たり"].iter().chain(POLITE) {
                rule(format!("{masu}{ending}"), base, TYPED, VK, false);
            }
            rule(format!("{masu}て"), base, TE, VK, false);
            rule(format!("{ba}れば"), base, TYPED, VK, false);
            for ending in ["られる", "させる"] { rule(format!("{nai}{ending}"), base, V1, VK, true); }
        }

        // I-adjectives: 儚 + ending ← 儚い.
        for ending in ["く", "くて", "かった", "かったら", "ければ", "かろう"] {
            rule(ending.into(), "い", TYPED, ADJ, false);
        }
        rule("くない".into(), "い", ADJ, ADJ, false);
        rule("そう".into(), "い", TYPED, ADJ, true);
        rule("すぎる".into(), "い", V1, ADJ, true);

        // ている and てる conjugate like ichidan verbs and lead back to the te-form.
        rule("ている".into(), "て", V1, TE, false);
        rule("でいる".into(), "で", V1, TE, false);
        rule("てる".into(), "て", V1, TE, true);
        rule("でる".into(), "で", V1, TE, true);

        rules
    }
}

mod english {
    /// Base forms for an English word, each with whether it is doubtful.
    pub(super) fn base_forms(word: &str) -> Vec<(String, bool)> {
        let mut forms = Vec::new();
        if !word
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '\'' || c == '’')
        {
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

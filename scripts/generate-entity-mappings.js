#!/usr/bin/env node
/**
 * Script to generate entity → base letter mappings for DIPLOMATIC normalization
 *
 * For diplomatic transcription in MENOTA:
 * - PRESERVE: Diacritics (á, ö, å, etc.) and Old Norse letters (ð, þ, æ, œ, ø)
 * - NORMALIZE: Special letter forms (open o, h with hook, insular, rotunda) → base letter
 *
 * Run with: bun scripts/generate-entity-mappings.js
 */

import { readFileSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Old Norse / meaningful letters that should be preserved (not normalized to base)
const PRESERVE_LETTERS = new Set([
  'ð', 'Ð', 'þ', 'Þ', 'æ', 'Æ', 'œ', 'Œ', 'ø', 'Ø',
  'ƿ', 'Ƿ',  // wynn - meaningful in Old English
  'ȝ', 'Ȝ',  // yogh - meaningful
]);

// Diacritics categorized for Old Norse diplomatic normalization:
// PRESERVE: Keep the diacriticized form as-is
const PRESERVE_DIACRITICS = new Set([
  'acute',           // á, é - vowel length
  'macr', 'macron',  // ā, ē - vowel length (alternative)
  'diaer', 'uml',    // ö, ü - vowel quality
  'ogon', 'ogonek',  // ą, ę - used in some transliteration standards
  'cedil',           // ç - variant of ogonek, keep
  'ring',            // å - Scandinavian vowel (keep for now)
]);

// NORMALIZE_TO_ACUTE: Convert to acute form
const NORMALIZE_TO_ACUTE = new Set([
  'grave',           // à → á (normalize to acute)
  'dblac',           // ő → ó (double acute → single acute)
]);

// STRIP_DIACRITICS: Remove diacritic, keep base letter only
const STRIP_DIACRITICS = new Set([
  'circ', 'circumflex',  // â → a
  'tilde',               // ã → a
  'breve',               // ă → a
  'dot',                 // ċ → c (scribal abbreviation, not phonetic)
  'dotbl',               // ḅ → b
  'caron',               // č → c
]);

// Combined set for modifier detection
const DIACRITIC_MODIFIERS = new Set([
  ...PRESERVE_DIACRITICS,
  ...NORMALIZE_TO_ACUTE,
  ...STRIP_DIACRITICS,
]);

// Form modifiers - these indicate orthographic variants that should normalize to base letter
const FORM_MODIFIERS = new Set([
  // Variant forms
  'open', 'close', 'turned', 'reversed', 'inv', 'inverted',
  'hook', 'lfhook', 'rthook', 'lefthook', 'righthook',
  'tail', 'descend', 'curl', 'curly', 'loop', 'squirrel',
  'insular', 'ins', 'rot', 'rotunda', 'uncial', 'unc',
  'scap', 'smallcap', 'cap', 'capital',
  'stroke', 'strok', 'slash', 'bar', 'cross', 'dbl', 'dblstrok',
  'long', 'short', 'high', 'low', 'mid', 'enl', 'enlongated',
  'half', 'script', 'flour', 'flourish', 'stem', 'arm', 'leg',
  'welsh', 'angl', 'visigot', 'med', 'medunc',
  'neckless', 'armless',
  'fin', 'rfin', 'lfin',  // final forms
  'sup', 'sub', 'super', 'subscr',  // positional variants
]);

// Special letter prefixes (order matters - longer matches first)
const SPECIAL_LETTERS = {
  // Old Norse / Germanic - PRESERVE these
  'thorn': 'þ',
  'eth': 'ð',
  'wynn': 'ƿ',
  'yogh': 'ȝ',
  'ezh': 'ʒ',
  'eng': 'ŋ',

  // Extended Latin
  'schwa': 'ə',
  'glottal': 'ʔ',

  // Special forms - NORMALIZE these to base letter
  'slong': 's',   // long s → s
  'slongs': 's',  // long s → s
  'rrot': 'r',    // rotunda r → r
  'drot': 'd',    // rotunda d → d
  'trot': 't',    // rotunda t → t

  // Special letter names
  'inodot': 'i',  // dotless i → i
  'jnodot': 'j',  // dotless j → j
  'szlig': 'ss',  // sharp s (ß) → ss for normalization
  'hwair': 'hv',  // hw digraph
};

// Greek letters (only match exact or with known suffixes)
const GREEK_LETTERS = {
  'alpha': 'α', 'beta': 'β', 'gamma': 'γ', 'delta': 'δ', 'epsilon': 'ε',
  'zeta': 'ζ', 'theta': 'θ', 'iota': 'ι', 'kappa': 'κ', 'lambda': 'λ',
  'mu': 'μ', 'nu': 'ν', 'xi': 'ξ', 'omicron': 'ο', 'pi': 'π', 'rho': 'ρ',
  'sigma': 'σ', 'tau': 'τ', 'upsilon': 'υ', 'phi': 'φ', 'chi': 'χ',
  'psi': 'ψ', 'omega': 'ω',
};

// Known ligatures - separate meaningful vs orthographic
// Meaningful ligatures (preserved): æ, œ
// Orthographic ligatures (normalized to component letters): ff, fi, st, ct, etc.
const MEANINGFUL_LIGATURES = {
  'aelig': 'æ',
  'oelig': 'œ',
  'oeopen': 'œ',
  'oeligt': 'œ',
};

const ORTHOGRAPHIC_LIGATURES = {
  'uelig': 'ue',
  'ijlig': 'ij',
  'fflig': 'ff',
  'filig': 'fi',
  'fllig': 'fl',
  'ffilig': 'ffi',
  'ffllig': 'ffl',
  'stlig': 'st',
  'ctlig': 'ct',
  'aalig': 'aa',
  'aolig': 'ao',
  'oolig': 'oo',
  'vylig': 'vy',
  'yylig': 'yy',
};

// Combined for pattern matching
const LIGATURES = { ...MEANINGFUL_LIGATURES, ...ORTHOGRAPHIC_LIGATURES };

// Modifiers that can follow a base letter
const MODIFIERS = [
  // Diacritics
  'acute', 'grave', 'circ', 'circumflex', 'tilde', 'macr', 'macron',
  'breve', 'dot', 'diaer', 'uml', 'ring', 'dblac', 'caron', 'cedil',
  'ogon', 'ogonek', 'hook', 'horn',

  // Position modifiers
  'bl', 'dotbl', 'above', 'below', 'sup', 'sub', 'super', 'subscr',

  // Style modifiers
  'scap', 'smallcap', 'cap', 'capital',
  'stroke', 'strok', 'slash', 'bar', 'cross', 'curl', 'curly',
  'tail', 'descend', 'insular', 'rot', 'rotunda',
  'open', 'close', 'turned', 'reversed', 'inv', 'inverted',
  'long', 'short', 'high', 'low', 'mid',

  // Ligature indicators
  'lig', 'liga', 'vlig', 'nlig', 'rlig', 'slig', 'tlig', 'mlig',

  // Abbreviation markers
  'enl', 'enlongated', 'abbr', 'abbrev',

  // Medieval specific
  'des', 'deslig', 'arm', 'med', 'ovlmed', 'tironian',
  'neckless', 'armless', 'red', 'uncial',

  // Combining indicators
  'comb', 'combining',

  // Numbers
  '1', '2', '3', '4', '5', '6', '7', '8', '9', '0',
];

// Entities that should normalize to empty (combining marks, etc.)
const COMBINING_PATTERNS = [
  /^comb/,      // combining marks
  /^uni0[23]/,  // Unicode combining range
  /^bar[ab]/,   // barbl, baracr - combining bars
];

// Entities to skip (not letters)
const SKIP_PATTERNS = [
  /^punct/,     // punctuation
  /^num/,       // numbers/numerals
  /^arr/,       // arrows
  /^box/,       // box drawing
  /^block/,     // block elements
  /^geo/,       // geometric shapes
  /^sym/,       // symbols
  /^math/,      // math symbols
  /^music/,     // music symbols
  /^currency/,  // currency
  /^bra/,       // brackets
  /^par/,       // parentheses
  /^quot/,      // quotes
  /^dash/,      // dashes
  /^dot[^a-z]/, // dots (but not dota, dotb, etc.)
  /^space/,     // spaces
  /^blank/,     // blanks
  /^null/,      // null
  /^del$/,      // delete
  /^esc$/,      // escape
  /^tab$/,      // tab
  /^cr$/,       // carriage return
  /^lf$/,       // line feed
  /^ff$/,       // form feed (but not fflig)
  /^bel$/,      // bell
  /^category$/, // metadata field

  // === RUNES (non-Latin scripts) ===
  // Patterns must be specific to avoid matching common words like 'thorn', 'slong'
  /YG[LS]?$/,         // Younger Futhark runes (YG, YGL, YGS)
  /YG[LS]?[a-z0-9]+$/i,  // Younger runes with modifiers (YGLcv, YGSdot, etc.)
  /[A-Z]MD[a-z0-9]*$/,   // Medieval runes (aMD, bMD, etc.) - must have uppercase prefix
  /[a-z]MD$/,            // Medieval runes ending with MD
  /OE$/,              // Old English runes (Futhorc) - exact match
  /[A-Z]OR$/,         // Older Futhark runes - must have uppercase prefix (aOR, bOR)
  /[A-Z]OR[a-z0-9]+$/,   // Older Futhark with modifiers (aORcv, etc.)
  /EL$/,              // Elder Futhark runes
  /[A-Z]GE[a-z0-9]*$/,   // General runes - must have uppercase prefix (aGE, aGEcv)
  /[A-Z]AF[a-z]*$/,      // Anglo-Frisian runes - must have uppercase prefix
  /[A-Z]SL[a-z]*$/,      // Staveless runes - must have uppercase prefix (aSL, etc.)
  /^LIG[a-z]+/,       // Ligated runes (LIGaYGfGEcv, etc.)
  /Medrun/,           // Medieval runic
  /\d+br/,            // Runes with branch counts (3br, 4br, etc.)
  /\d+brk/,           // Runes with break counts (3brk, 4brk, etc.)
  /\d+sh/,            // Runes with shape numbers (4sh, etc.)
  /\d+acr/,           // Runes with across patterns
  /\d+lf/,            // Runes with left patterns

  // === PUNCTUATION/SYMBOLS ===
  /PM$/,              // punctuation marks (middotPM, twodotPM)
  /mod$/,             // modifiers (apomod, etc.)
  /^tri/,             // triangle punctuation
  /^verbar/,          // vertical bar variants
  /^metr/,            // metrical symbols
  /^rom/,             // roman numerals/signs
  /^punc/,            // punctuation
  /^posit/,           // positura
  /^col/,             // colon variants
  /^virg/,            // virgula
  /sp$/,              // space variants (emsp, hairsp, etc.)
  /^emsp\d/,          // em space variants (emsp13, emsp14, emsp16)
  /^hedera/,          // decorative elements
  /^squar/,           // square symbols
  /^circ/,            // circle symbols
  /^loz/,             // lozenge
  /^renvoi/,          // signe de renvoi
  /^refmark/,         // reference marks
  /^dotcross/,        // dotted cross
  /^fracsol/,         // fraction slash
  /^hyphpoint/,       // hyphenation point
  /^sgldr/,           // dot leader
  /^quaddot/,         // quad dots
  /^dipledot/,        // diple dot
  /^infin/,           // infinity
  /^logand/,          // logical and
  /^notequals/,       // not equals

  // === CURRENCY/UNITS ===
  /germ$/,            // German currency (pennygerm, schillgerm)
  /germ[a-z]+$/,      // German script variants
  /^penny/,           // penny signs
  /^grosch/,          // groschen
  /^libra/,           // libra signs
  /^lira/,            // lira signs
  /^mark(old|flour)?$/, // mark signs
  /^schil/,           // schilling
  /^scudi/,           // scudi
  /^sestert/,         // sestertius
  /^sextans/,         // sextans
  /^ounce/,           // ounce signs
  /^scruple/,         // scruple
  /^dram$/,           // dram
  /^lbbar/,           // lb bar

  // === ABBREVIATION SIGNS (not letters) ===
  /base$/,            // USbase, usbase, CONbase, conbase
  /slash$/,           // ETslash, etslash
  /^autem$/,          // autem abbreviation sign
  /^arbar$/,          // combining abbreviation marks
  /^erang$/,
  /^ercurl$/,
  /^erhigh$/,
  /^eracute$/,
  /^rabar$/,
  /^urrot$/,
  /^urlemn$/,
  /^ersub$/,          // combining zigzag below

  // === MISC SYMBOLS ===
  /^mufi/,            // MUFI-specific symbols
  /^Psalmu/i,         // Psalmus/Psalmi signs
  /^Psalm/i,          // Psalmi signs
  /^smallzero/,       // small zero
  /^flor/,            // floren signs
  /^msign/,           // m signs
  /^penning/,         // penning
  /^reichtal/,        // reichstaler
  /^Rslstrok/,        // Response sign
  /^Vslstrok/,        // Versicle sign
  /^CON/,             // Roman numeral reversed
  /^con/,             // Small reversed c (when standalone)

  // === BRACKETS AND HYPHENS ===
  /sqb$/,             // square brackets (luhsqb, ruhsqb, llhsqb, rlhsqb)
  /sqbNT$/,           // substitution markers
  /brack$/,           // brackets (lUbrack, rUbrack)
  /dblpar$/,          // double parentheses
  /hyph$/,            // hyphens (dblhyph, dbloblhyph)
  /^bla.*bul$/,       // bullets (blaleftbul, blarightbul)

  // === PUNCTUATION PATTERNS ===
  /PM$/i,             // punctuation marks (case-insensitive)
  /punc$/,            // punc endings
  /punct$/,           // punct endings
  /^run.*punc/,       // runic punctuation
  /dots$/,            // dot patterns (fivedots, quaddots)
  /dotsm$/,           // dot mark patterns
  /^ver.*dots$/,      // vertical dots

  // === RELIGIOUS/MUSICAL SIGNS ===
  /^Antiphon/i,       // Antiphon
  /^Hymnus/i,         // Hymnus
  /^ramus$/,          // palm branch
  /^ductsimpl$/,      // ductus simplex

  // === MISC PUNCTUATION ===
  /^hidot$/,          // high dot
  /^midring$/,        // middle ring
  /^medcom$/,         // medieval comma
  /^wavylin$/,        // wavy line
  /^semicolondot$/,   // semicolon with dot
  /^bidots/,          // bidots patterns

  // === ROMAN NUMERALS ===
  /numbar$/,          // Roman numeral bars (Cnumbar, cnumbar)

  // === COMBINING MARKS (additional) ===
  /^macrhigh$/,       // combining high macron
  /^macrmed$/,        // combining medium-high macron
  /^ovlhigh$/,        // combining high overline
  /^ovlmed$/,         // combining medium-high overline

  // === MISC MARKERS ===
  /NT$/,              // Notation markers (luhsqbNT)
];

// Common symbol/punctuation names to skip
const SKIP_NAMES = new Set([
  'amp', 'lt', 'gt', 'quot', 'apos',  // XML entities
  'percnt', 'percent', 'dollar', 'pound', 'curren', 'currency', 'yen', 'euro', 'cent',
  'comma', 'period', 'colon', 'semi', 'semicolon', 'excl', 'quest', 'question',
  'hyphen', 'minus', 'plus', 'equals', 'ast', 'asterisk', 'sol', 'bsol',
  'lpar', 'rpar', 'lsqb', 'rsqb', 'lcub', 'rcub', 'lbrace', 'rbrace',
  'lowbar', 'underscore', 'grave', 'caret', 'tilde', 'verbar', 'pipe',
  'commat', 'at', 'hash', 'num', 'number',
  'laquo', 'raquo', 'lsaquo', 'rsaquo', 'ldquo', 'rdquo', 'lsquo', 'rsquo',
  'iexcl', 'iquest', 'sect', 'para', 'pilcrow', 'dagger', 'ddagger',
  'plusmn', 'times', 'divide', 'frac12', 'frac14', 'frac34', 'deg', 'prime',
  'micro', 'middot', 'cedil', 'copy', 'reg', 'trade',
  'bull', 'bullet', 'hellip', 'mdash', 'ndash', 'horbar',
  'cir', 'circ', 'ring', 'check', 'cross', 'star',
  'sp', 'nbsp', 'ensp', 'emsp', 'thinsp', 'zwsp', 'zwnj', 'zwj',
  'shy', 'rlm', 'lrm', 'bom',
  // Standalone diacritics/modifiers (not letter+modifier)
  'acute', 'grave', 'uml', 'macr', 'breve', 'dot', 'ring', 'caron', 'cedil', 'ogon',
  'tilde', 'diaer', 'circ', 'horn', 'hook',
  // Misc symbols
  'not', 'sup1', 'sup2', 'sup3', 'ordf', 'ordm', 'tld', 'brvbar',
  'fnof', 'weierp', 'image', 'real', 'alefsym',
  // Brackets and punctuation
  'lsqbqu', 'rsqbqu', 'lwhsqb', 'rwhsqb', 'langb', 'rangb',
  'lsquolow', 'rsquorev', 'ldquolow', 'rdquorev', 'permil', 'revpara',
  // Spaces and formatting
  'emsp13', 'emsp14', 'emsp16', 'hairsp', 'puncsp', 'figsp',
  // Misc punctuation/symbols (from review)
  'obol', 'ecu', 'dblsol', 'dblcross', 'nbhy', 'enqd', 'emqd', 'dblldr',
  // Notation markers
  'luhsqbnt',  // Note: lowercase for matching
]);

// Standard XML entities
const XML_ENTITIES = ['amp', 'lt', 'gt', 'quot', 'apos'];

/**
 * Determine if a character should be preserved (Old Norse letters, etc.)
 */
function shouldPreserveChar(char) {
  if (!char) return false;
  return PRESERVE_LETTERS.has(char);
}

/**
 * Map of base letters to their acute forms
 */
const ACUTE_FORMS = {
  'a': 'á', 'A': 'Á',
  'e': 'é', 'E': 'É',
  'i': 'í', 'I': 'Í',
  'o': 'ó', 'O': 'Ó',
  'u': 'ú', 'U': 'Ú',
  'y': 'ý', 'Y': 'Ý',
  'æ': 'ǽ', 'Æ': 'Ǽ',
  'ø': 'ǿ', 'Ø': 'Ǿ',
};

/**
 * Determine the diplomatic normalization for a letter with diacritics
 * @param {string} baseLetter - The base letter (e.g., 'a', 'e')
 * @param {string} modifier - The diacritic modifier (e.g., 'grave', 'circ')
 * @param {string} char - The actual Unicode character
 * @param {boolean} isUpperCase - Whether the original was uppercase
 * @returns {string} The normalized form
 */
function normalizeDiacritic(baseLetter, modifier, char, isUpperCase) {
  const lowerMod = modifier.toLowerCase();

  // Check each category
  for (const mod of PRESERVE_DIACRITICS) {
    if (lowerMod.includes(mod)) {
      // Preserve the actual character
      return char || (isUpperCase ? baseLetter.toUpperCase() : baseLetter);
    }
  }

  for (const mod of NORMALIZE_TO_ACUTE) {
    if (lowerMod.includes(mod)) {
      // Convert to acute form
      const base = isUpperCase ? baseLetter.toUpperCase() : baseLetter.toLowerCase();
      return ACUTE_FORMS[base] || base;
    }
  }

  for (const mod of STRIP_DIACRITICS) {
    if (lowerMod.includes(mod)) {
      // Strip diacritic, return base letter
      return isUpperCase ? baseLetter.toUpperCase() : baseLetter.toLowerCase();
    }
  }

  // Default: preserve char if available
  return char || baseLetter;
}

/**
 * Analyze modifiers and determine the primary diacritic type
 */
function getPrimaryDiacritic(modifierString) {
  const lower = modifierString.toLowerCase();

  // Check in order of specificity
  const allDiacritics = [
    ...STRIP_DIACRITICS,
    ...NORMALIZE_TO_ACUTE,
    ...PRESERVE_DIACRITICS,
  ];

  for (const diac of allDiacritics) {
    if (lower.includes(diac)) {
      return diac;
    }
  }
  return null;
}

/**
 * Check if entity name has ONLY diacritic modifiers (no form modifiers)
 * Returns { hasDiacritics, hasForms } to help decide normalization
 */
function analyzeModifiers(remainder) {
  if (!remainder) return { hasDiacritics: false, hasForms: false };

  let rest = remainder.toLowerCase();
  let hasDiacritics = false;
  let hasForms = false;

  while (rest.length > 0) {
    let matched = false;

    // Check diacritic modifiers first
    for (const mod of DIACRITIC_MODIFIERS) {
      if (rest.startsWith(mod)) {
        rest = rest.slice(mod.length);
        hasDiacritics = true;
        matched = true;
        break;
      }
    }

    if (!matched) {
      // Check form modifiers
      for (const mod of FORM_MODIFIERS) {
        if (rest.startsWith(mod)) {
          rest = rest.slice(mod.length);
          hasForms = true;
          matched = true;
          break;
        }
      }
    }

    if (!matched) {
      // Check for numbers or other patterns
      if (/^[0-9]/.test(rest)) {
        rest = rest.slice(1);
        matched = true;
      } else if (rest.length <= 3) {
        // Allow short remainders
        break;
      } else {
        break;
      }
    }
  }

  return { hasDiacritics, hasForms };
}

/**
 * Get base letter for diplomatic normalization
 * - Preserves diacritics (á, ö, å)
 * - Preserves Old Norse letters (ð, þ, æ)
 * - Normalizes form variants (open o → o, h with hook → h)
 */
function getBaseLetterForDiplomatic(baseLetter, char, hasForms) {
  // If no form modifiers, preserve the actual character (with diacritics)
  if (!hasForms && char) {
    return char;
  }

  // If we have form modifiers, normalize to base letter
  // BUT preserve Old Norse letters even with form modifiers
  if (char && shouldPreserveChar(char)) {
    return char;
  }

  // For form variants, return just the base letter
  return baseLetter;
}

function extractBaseLetter(entityName, entityData) {
  const name = entityName.toLowerCase();
  const char = entityData.char;  // The actual Unicode character

  // Skip known symbol/punctuation names
  if (SKIP_NAMES.has(name)) {
    return { base: null, confidence: 'skip', method: 'skip-name' };
  }

  // Skip non-letter entities by pattern
  for (const pattern of SKIP_PATTERNS) {
    if (pattern.test(name)) {
      return { base: null, confidence: 'skip', method: 'skip-pattern' };
    }
  }

  // Combining marks normalize to empty
  for (const pattern of COMBINING_PATTERNS) {
    if (pattern.test(name)) {
      return { base: '', confidence: 'high', method: 'combining-mark' };
    }
  }

  // Handle complex ligatures FIRST (before special letters)
  // e.g., slongtlig → st, thornXlig → þX, etc.
  const complexLigPatterns = [
    { prefix: 'slong', base: 's' },   // long s → s
    { prefix: 'thorn', base: 'þ' },   // preserve þ (meaningful)
    { prefix: 'eth', base: 'ð' },     // preserve ð (meaningful)
    { prefix: 'drot', base: 'd' },    // d rotunda → d
    { prefix: 'trot', base: 't' },    // t rotunda → t
    { prefix: 'rrot', base: 'r' },    // r rotunda → r
  ];
  for (const { prefix, base: prefixBase } of complexLigPatterns) {
    if (name.startsWith(prefix) && name.includes('lig')) {
      // Extract the second component of the ligature
      const remainder = name.slice(prefix.length);
      const ligIndex = remainder.indexOf('lig');
      if (ligIndex > 0) {
        const secondPart = remainder.slice(0, ligIndex);
        // Normalize both parts
        return {
          base: prefixBase + secondPart[0].toLowerCase(),
          confidence: 'medium',
          method: 'complex-ligature-normalized',
          detail: `${prefix}+${secondPart}lig → ${prefixBase}${secondPart[0]}`
        };
      }
      // Just the prefix with lig (no second part)
      return {
        base: prefixBase,
        confidence: 'medium',
        method: 'complex-ligature-normalized',
        detail: `${prefix}lig → ${prefixBase}`
      };
    }
  }

  // Check special letters (longest match first)
  const sortedSpecials = Object.keys(SPECIAL_LETTERS).sort((a, b) => b.length - a.length);
  for (const special of sortedSpecials) {
    if (name.startsWith(special)) {
      const remainder = name.slice(special.length);
      // Verify remainder looks like modifiers
      if (remainder === '' || looksLikeModifiers(remainder)) {
        // For special letters, use the mapping (which normalizes forms like slong → s)
        // But check if this is a preserved letter (eth, thorn)
        let baseMapping = SPECIAL_LETTERS[special];
        const { hasDiacritics, hasForms } = analyzeModifiers(remainder);

        // Preserve case from original entity name for preserved letters
        const isUpperCase = /^[A-Z]/.test(entityName);
        if (shouldPreserveChar(baseMapping) && isUpperCase) {
          baseMapping = baseMapping.toUpperCase();
        }

        // For preserved letters with diacritics, use the actual char
        if (shouldPreserveChar(SPECIAL_LETTERS[special]) && hasDiacritics && char) {
          return {
            base: char,
            confidence: 'high',
            method: 'special-letter-diacritic',
            detail: `${special} + diacritics → ${char}`
          };
        }

        // Otherwise use the base mapping (normalizes slong → s, etc.)
        return {
          base: baseMapping,
          confidence: 'high',
          method: 'special-letter',
          detail: `${special} + ${remainder || '(none)'} → ${baseMapping}`
        };
      }
    }
  }

  // Check Greek letters (exact match or with modifiers)
  const sortedGreek = Object.keys(GREEK_LETTERS).sort((a, b) => b.length - a.length);
  for (const greek of sortedGreek) {
    if (name.startsWith(greek)) {
      const remainder = name.slice(greek.length);
      if (remainder === '' || looksLikeModifiers(remainder)) {
        const baseChar = char || GREEK_LETTERS[greek];
        return {
          base: baseChar,
          confidence: 'medium',
          method: 'greek-letter',
          detail: `${greek} + ${remainder || '(none)'}`
        };
      }
    }
  }

  // Handle superscript forms: use char if available
  const supMatch = name.match(/^([a-z]+?)(a|e|i|o|u|r|s|m|n|t|v|w|z)?sup$/);
  if (supMatch && char) {
    return {
      base: char,
      confidence: 'medium',
      method: 'superscript',
      detail: `superscript form`
    };
  }

  // Check known ligatures
  // For meaningful ligatures (æ, œ): preserve with diacritics from char if available
  // For orthographic ligatures (ff, st): always expand to component letters
  for (const [ligName, ligChar] of Object.entries(MEANINGFUL_LIGATURES)) {
    if (name.startsWith(ligName)) {
      const remainder = name.slice(ligName.length);
      if (remainder === '' || looksLikeModifiers(remainder)) {
        // Use actual char to preserve any diacritics (e.g., aeligacute → ǽ)
        const baseChar = char || ligChar;
        return {
          base: baseChar,
          confidence: 'high',
          method: 'meaningful-ligature',
          detail: `${ligName} → ${baseChar}`
        };
      }
    }
  }

  for (const [ligName, expanded] of Object.entries(ORTHOGRAPHIC_LIGATURES)) {
    if (name.startsWith(ligName)) {
      const remainder = name.slice(ligName.length);
      if (remainder === '' || looksLikeModifiers(remainder)) {
        // Always expand orthographic ligatures to component letters
        return {
          base: expanded,
          confidence: 'high',
          method: 'orthographic-ligature-normalized',
          detail: `${ligName} → ${expanded}`
        };
      }
    }
  }

  // Handle turned letters: Xturn or turnX → base letter X (form variant → normalized)
  const turnMatch = name.match(/^([a-z])turn$/i) || name.match(/^turn([a-z])$/i);
  if (turnMatch) {
    const baseLetter = turnMatch[1].toLowerCase();
    const isUpperCase = /^[A-Z]/.test(entityName[0]);
    return {
      base: isUpperCase ? baseLetter.toUpperCase() : baseLetter,
      confidence: 'medium',
      method: 'turned-letter-normalized',
      detail: `turned ${baseLetter} → ${baseLetter}`
    };
  }

  // Handle XXturn (two-letter turned, like "ginsturn") → base letter
  const turnMatch2 = name.match(/^([a-z]+)turn$/i);
  if (turnMatch2) {
    const baseLetter = turnMatch2[1][0].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'turned-letter-complex-normalized',
      detail: `turned form → ${baseLetter}`
    };
  }

  // Handle Welsh letters: Xwelsh → base letter (form variant)
  const welshMatch = name.match(/^([a-z]+)welsh$/i);
  if (welshMatch) {
    const baseLetter = welshMatch[1][0].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'welsh-letter-normalized',
      detail: `Welsh ${welshMatch[1]} → ${baseLetter}`
    };
  }

  // Handle two-letter ligatures: XXlig (aa, ao, oo, ll, pp, uu, yy, etc.)
  // For diplomatic: normalize to the component letters (except ae, oe which are meaningful)
  const twoLetterLigMatch = name.match(/^([a-z])([a-z])lig(.*)$/i);
  if (twoLetterLigMatch) {
    const [, first, second, rest] = twoLetterLigMatch;
    // Check if rest is empty or looks like modifiers
    if (rest === '' || looksLikeModifiers(rest)) {
      const combo = (first + second).toLowerCase();

      // ae and oe ligatures are meaningful in Old Norse → preserve
      if (combo === 'ae' || combo === 'oe') {
        const preservedChar = combo === 'ae' ? 'æ' : 'œ';
        return {
          base: preservedChar,
          confidence: 'high',
          method: 'meaningful-ligature',
          detail: `${combo}lig → ${preservedChar}`
        };
      }

      // Other ligatures are just orthographic → normalize to component letters
      return {
        base: first.toLowerCase() + second.toLowerCase(),
        confidence: 'medium',
        method: 'ligature-normalized',
        detail: `${first}${second}lig → ${first}${second}`
      };
    }
  }

  // Handle hook letters: Xhook, Xlfhook, etc. → base letter (form variant → normalized)
  const hookMatch = name.match(/^([a-z]+?)(lf|rt|left|right)?hook(.*)$/i);
  if (hookMatch) {
    const [, letterPart, , rest] = hookMatch;
    if (letterPart.length <= 3 && (rest === '' || looksLikeModifiers(rest))) {
      const baseLetter = letterPart[0].toLowerCase();
      return {
        base: baseLetter,
        confidence: 'medium',
        method: 'hook-letter-normalized',
        detail: `${letterPart} with hook → ${baseLetter}`
      };
    }
  }

  // Handle loop letters: Xloop → base letter (form variant)
  const loopMatch = name.match(/^([a-z])loop$/i);
  if (loopMatch) {
    const baseLetter = loopMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'loop-letter-normalized',
      detail: `${baseLetter} with loop → ${baseLetter}`
    };
  }

  // Handle leg variants: Xlrleg, Xleg → base letter (form variant)
  const legMatch = name.match(/^([a-z])(lr|l|r)?leg$/i);
  if (legMatch) {
    const baseLetter = legMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'leg-letter-normalized',
      detail: `${baseLetter} with leg → ${baseLetter}`
    };
  }

  // Handle script variants: Xscript → base letter (form variant)
  const scriptMatch = name.match(/^([a-z])script$/i);
  if (scriptMatch) {
    const baseLetter = scriptMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'script-letter-normalized',
      detail: `script ${baseLetter} → ${baseLetter}`
    };
  }

  // Handle half letters: Xhalf → base letter (form variant)
  const halfMatch = name.match(/^([a-z])half$/i);
  if (halfMatch) {
    const baseLetter = halfMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'half-letter-normalized',
      detail: `half ${baseLetter} → ${baseLetter}`
    };
  }

  // Handle flourish letters: Xflour → base letter (form variant)
  const flourMatch = name.match(/^([a-z])flour(.*)$/i);
  if (flourMatch) {
    const baseLetter = flourMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'flourish-letter-normalized',
      detail: `${baseLetter} with flourish → ${baseLetter}`
    };
  }

  // Handle stem letters: Xstem → base letter (form variant)
  const stemMatch = name.match(/^([a-z])stem$/i);
  if (stemMatch) {
    const baseLetter = stemMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'stem-letter-normalized',
      detail: `${baseLetter} with stem → ${baseLetter}`
    };
  }

  // Handle insular forms: Xins, fins, vins, etc. → base letter (form variant)
  const insMatch = name.match(/^([a-z]+)ins(.*)$/i);
  if (insMatch) {
    const [, base, rest] = insMatch;
    if (base.length <= 2 && (rest === '' || looksLikeModifiers(rest))) {
      const baseLetter = base[0].toLowerCase();
      return {
        base: baseLetter,
        confidence: 'medium',
        method: 'insular-letter-normalized',
        detail: `insular ${base} → ${baseLetter}`
      };
    }
  }

  // Handle uncial forms: Xunc, munc, etc. → base letter (form variant)
  const uncMatch = name.match(/^([a-z]+)unc(.*)$/i);
  if (uncMatch) {
    const [, base, rest] = uncMatch;
    if (base.length <= 2 && (rest === '' || looksLikeModifiers(rest))) {
      const baseLetter = base[0].toLowerCase();
      return {
        base: baseLetter,
        confidence: 'medium',
        method: 'uncial-letter-normalized',
        detail: `uncial ${base} → ${baseLetter}`
      };
    }
  }

  // Handle regional forms: Xangl (Anglicana), Xvisigot (Visigothic) → base letter
  const regionalMatch = name.match(/^([a-z])(angl|visigot)$/i);
  if (regionalMatch) {
    const baseLetter = regionalMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'regional-letter-normalized',
      detail: `regional form ${baseLetter} → ${baseLetter}`
    };
  }

  // Handle letters with overlines: Xovlhigh, Xovlmed → base letter (form variant)
  const ovlMatch = name.match(/^([a-z])ovl(high|med)$/i);
  if (ovlMatch) {
    const baseLetter = ovlMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'overline-letter-normalized',
      detail: `${baseLetter} with overline → ${baseLetter}`
    };
  }

  // Handle letters with descenders: Xrdes, Xldes, Xdes → base letter (form variant)
  const desMatch = name.match(/^([a-z]+)(r|l)?des$/i);
  if (desMatch) {
    const [, base] = desMatch;
    if (base.length <= 3) {
      const baseLetter = base[0].toLowerCase();
      return {
        base: baseLetter,
        confidence: 'medium',
        method: 'descender-letter-normalized',
        detail: `${base} with descender → ${baseLetter}`
      };
    }
  }

  // Handle stroke variants: Xstrleg, Xstrok, Xdiagstrok → base letter (form variant)
  const strokeMatch = name.match(/^([a-z])(str|strok|diagstrok)(.*)$/i);
  if (strokeMatch) {
    const baseLetter = strokeMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'stroke-letter-normalized',
      detail: `${baseLetter} with stroke → ${baseLetter}`
    };
  }

  // Handle OBIIT (O with long stroke) and similar → base letter
  const obiitMatch = name.match(/^obiit$/i);
  if (obiitMatch) {
    return {
      base: 'o',
      confidence: 'medium',
      method: 'special-form-normalized',
      detail: `obiit → o`
    };
  }

  // Handle ET forms: ETfin, etfin → base letters (meaningful abbreviation sign, keep as-is)
  const etMatch = name.match(/^et(fin|all)?$/i);
  if (etMatch && char) {
    return {
      base: char,
      confidence: 'medium',
      method: 'et-form',
      detail: `et form → ${char}`
    };
  }

  // Handle squirrel tail: Xsquirrel → base letter (form variant)
  const squirrelMatch = name.match(/^([a-z])squirrel$/i);
  if (squirrelMatch) {
    const baseLetter = squirrelMatch[1].toLowerCase();
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'squirrel-letter-normalized',
      detail: `${baseLetter} with squirrel tail → ${baseLetter}`
    };
  }

  // Handle MHG (Middle High German) forms: mhgzed → base letter (form variant)
  const mhgMatch = name.match(/^mhg(.+)$/i);
  if (mhgMatch) {
    // Extract the base letter from the name (e.g., mhgzed → z)
    const letterPart = mhgMatch[1].toLowerCase();
    const baseLetter = letterPart[0];
    return {
      base: baseLetter,
      confidence: 'medium',
      method: 'mhg-letter-normalized',
      detail: `MHG ${letterPart} → ${baseLetter}`
    };
  }

  // Try single letter extraction (a-z) - Apply diplomatic normalization rules
  const firstChar = name[0];
  if (/^[a-z]$/.test(firstChar)) {
    const remainder = name.slice(1);

    // Check if remainder looks like modifiers (diacritics, etc.)
    if (remainder === '' || looksLikeModifiers(remainder)) {
      const { hasDiacritics, hasForms } = analyzeModifiers(remainder);

      // Determine the base letter (preserve case from original entity name)
      const isUpperCase = /^[A-Z]/.test(entityName[0]);
      const baseLetter = isUpperCase ? firstChar.toUpperCase() : firstChar;

      if (hasForms) {
        // Has form modifiers (open, hook, insular, etc.) → normalize to base letter
        // UNLESS the char itself is a preserved letter (ð, þ, æ)
        if (char && shouldPreserveChar(char)) {
          return {
            base: char,
            confidence: 'medium',
            method: 'form-variant-preserved',
            detail: `${entityName} form variant, preserved: ${char}`
          };
        }
        // Normalize to base letter, possibly with diacritics applied
        // For now, just use base letter when form modifiers are present
        return {
          base: baseLetter,
          confidence: 'medium',
          method: 'form-variant-normalized',
          detail: `${entityName} → ${baseLetter} (form normalized)`
        };
      } else if (hasDiacritics) {
        // Has diacritic modifiers → apply diplomatic normalization rules
        const primaryDiac = getPrimaryDiacritic(remainder);

        if (primaryDiac) {
          const normalized = normalizeDiacritic(firstChar, primaryDiac, char, isUpperCase);

          // Determine which category this falls into for the method description
          let method = 'letter-with-diacritics';
          if (STRIP_DIACRITICS.has(primaryDiac)) {
            method = 'diacritic-stripped';
          } else if (NORMALIZE_TO_ACUTE.has(primaryDiac)) {
            method = 'diacritic-normalized-to-acute';
          } else if (PRESERVE_DIACRITICS.has(primaryDiac)) {
            method = 'diacritic-preserved';
          }

          return {
            base: normalized,
            confidence: 'high',
            method: method,
            detail: `${entityName} (${primaryDiac}) → ${normalized}`
          };
        }

        // Fallback: preserve the char if we couldn't identify the diacritic
        if (char) {
          return {
            base: char,
            confidence: 'medium',
            method: 'letter-with-diacritics-unknown',
            detail: `${entityName} → ${char}`
          };
        }
      } else if (char) {
        // No modifiers recognized, use char
        return {
          base: char,
          confidence: remainder === '' ? 'high' : 'medium',
          method: 'letter-char',
          detail: `${entityName} → ${char}`
        };
      }

      // Fallback to just the letter if no char available
      return {
        base: baseLetter,
        confidence: 'low',
        method: 'single-letter-no-char',
        detail: `${baseLetter} + ${remainder || '(none)'}`
      };
    }

    // Try with two-letter combinations for common ligatures
    if (remainder.length >= 1) {
      const secondChar = remainder[0];
      const twoLetter = firstChar + secondChar;
      const rem2 = remainder.slice(1);

      // Common letter combinations that form ligatures
      const ligatureStarts = ['ae', 'oe', 'av', 'et', 'an', 'ar', 'as', 'at', 'au', 'ay',
                             'en', 'er', 'es', 'in', 'on', 'or', 'us', 'um'];

      if (ligatureStarts.includes(twoLetter) && (rem2 === 'lig' || rem2.startsWith('lig'))) {
        // Use char if available
        if (char) {
          return {
            base: char,
            confidence: 'medium',
            method: 'letter-ligature',
            detail: `${twoLetter}lig → ${char}`
          };
        }
      }
    }
  }

  // Uppercase letters - try lowercase version but use original char
  if (/^[A-Z]/.test(entityName[0])) {
    const lower = entityName.toLowerCase();
    const result = extractBaseLetter(lower, { ...entityData, char: null }); // Test pattern without char
    if (result.base && result.confidence !== 'skip' && result.confidence !== 'unknown') {
      // Pattern matched - use the actual char to preserve case and diacritics
      if (char) {
        return {
          ...result,
          base: char,
          method: result.method + '-uppercase'
        };
      }
    }
  }

  // Fallback: use the actual character if available and looks like a letter
  if (char && char.length >= 1) {
    // Check if it's a letter-like character
    const normalized = char.normalize('NFD')[0];
    if (normalized && /[a-zA-Z]/.test(normalized)) {
      return {
        base: char,
        confidence: 'low',
        method: 'unicode-char',
        detail: `char: ${char}`
      };
    }
  }

  // Helper to check if char is in Private Use Area (no standard Unicode representation)
  const isPUA = (c) => {
    if (!c || c.length === 0) return true;
    const code = c.codePointAt(0);
    // BMP PUA: U+E000-U+F8FF, Supplementary PUA-A: U+F0000-U+FFFFF, PUA-B: U+100000-U+10FFFF
    return (code >= 0xE000 && code <= 0xF8FF) ||
           (code >= 0xF0000 && code <= 0xFFFFF) ||
           (code >= 0x100000 && code <= 0x10FFFF);
  };

  // Fallback for ligatures without standard chars: extract component letters from name
  // e.g., faumllig → fa, anscaplig → an, ffylig → ffy
  // Also handles PUA characters that don't have standard Unicode representations
  if (name.includes('lig') && (!char || isPUA(char))) {
    // Remove common suffixes and modifiers to get base letters
    // Order matters! More specific patterns first
    let basePart = name.toLowerCase()
      .replace(/lig$/, '')           // Remove final 'lig'
      // Special letter combinations (before removing individual parts)
      .replace(/aelig/g, 'æ')
      .replace(/oelig/g, 'œ')
      .replace(/thorn/g, 'þ')
      .replace(/eth/g, 'ð')
      .replace(/slong/g, 'ſ')
      .replace(/mhgzed/g, 'z')       // Middle High German zed
      // Rotunda forms
      .replace(/drot/g, 'd')
      .replace(/trot/g, 't')
      .replace(/rrot/g, 'r')
      // Small caps - keep the letter
      .replace(/nscap/g, 'n')
      .replace(/rscap/g, 'r')
      .replace(/scap/g, '')          // Remove remaining 'scap'
      // Other modifiers
      .replace(/enl(osmall?)?/g, '')  // Remove enlarged indicators (enl, enlosmall, enlosmal)
      .replace(/flour/g, '')         // Remove flourish
      .replace(/arm/g, '')           // Remove arm
      .replace(/neckless/g, '')      // Remove neckless
      .replace(/close/g, '')         // Remove close
      // Convert diacritic indicators to actual diacritics
      .replace(/auml/g, 'ä')
      .replace(/ouml/g, 'ö')
      .replace(/uuml/g, 'ü')
      .replace(/euml/g, 'ë')
      .replace(/iuml/g, 'ï')
      .replace(/yuml/g, 'ÿ')
      .replace(/aacute/g, 'á')
      .replace(/eacute/g, 'é')
      .replace(/iacute/g, 'í')
      .replace(/oacute/g, 'ó')
      .replace(/uacute/g, 'ú')
      .replace(/yacute/g, 'ý')
      .replace(/amacr/g, 'ā')
      .replace(/emacr/g, 'ē')
      .replace(/imacr/g, 'ī')
      .replace(/omacr/g, 'ō')
      .replace(/umacr/g, 'ū')
      .replace(/uml/g, '')           // Remove remaining uml indicators
      .replace(/macr/g, '')          // Remove remaining macron indicators
      .replace(/acute/g, '')         // Remove remaining acute indicators
      .replace(/dot/g, '');          // Remove dot indicator

    // Clean up any leftover patterns (keep letters and common diacritics)
    basePart = basePart.replace(/[^a-zæœþðſäöüëïÿáéíóúýāēīōū]/gi, '');

    if (basePart && basePart.length > 0 && basePart.length <= 4) {
      return {
        base: basePart,
        confidence: 'low',
        method: 'ligature-name-fallback',
        detail: `extracted from name: ${basePart}`
      };
    }
  }

  // Fallback for letter forms without standard chars: extract base letter
  // e.g., idblstrok → i, jdblstrok → j, burlemn → b, etc.
  if (!char || isPUA(char)) {
    // Try to extract single leading letter followed by form indicator
    const letterFormMatch = name.match(/^([a-z])(dbl|semi|vert|diag|comb|lemn|rfin|suptrot|loop|strok|rg|brevinv|medunc|scap[lr])/i);
    if (letterFormMatch) {
      const baseLetter = letterFormMatch[1].toLowerCase();
      return {
        base: baseLetter,
        confidence: 'low',
        method: 'letter-form-fallback',
        detail: `base letter from name: ${baseLetter}`
      };
    }

    // Try patterns like Xacombcirc (ea, eu combinations)
    const combMatch = name.match(/^([a-z]{2})comb/i);
    if (combMatch) {
      return {
        base: combMatch[1].toLowerCase(),
        confidence: 'low',
        method: 'comb-letter-fallback',
        detail: `letter combo: ${combMatch[1]}`
      };
    }

    // Try patterns like fins..., mmedunc..., drot..., etc.
    const prefixPatterns = [
      { pattern: /^fins(.*)$/i, base: 'f' },    // Insular f
      { pattern: /^vins(.*)$/i, base: 'v' },    // Insular v
      { pattern: /^mmedunc(.*)$/i, base: 'm' }, // Uncial m
      { pattern: /^munc(.*)$/i, base: 'm' },    // Uncial m
      { pattern: /^drot(.*)$/i, base: 'd' },    // D rotunda
      { pattern: /^trot(.*)$/i, base: 't' },    // T rotunda
      { pattern: /^do(acute)?suptrot$/i, base: 'd' }, // D + suptrot
      { pattern: /^nscap[lr]?des$/i, base: 'n' }, // Small cap N
      { pattern: /^g(div|lg|sm).*loop$/i, base: 'g' }, // G with loops
      { pattern: /^k(semi)?close$/i, base: 'k' }, // K close forms
      { pattern: /^yrg.*$/i, base: 'y' },       // Y variants
      { pattern: /^l(er)?fin$/i, base: 'l' },   // L final forms
      { pattern: /^([bhk])urlemn$/i, base: '$1' }, // urlemn forms - handled below
    ];

    // Special handling for Xurlemn pattern (b/h/k with ur lemniskate)
    const urlemnMatch = name.match(/^([bhk])urlemn$/i);
    if (urlemnMatch) {
      return {
        base: urlemnMatch[1].toLowerCase(),
        confidence: 'low',
        method: 'urlemn-fallback',
        detail: `${urlemnMatch[1]} with ur lemniskate`
      };
    }

    for (const { pattern, base } of prefixPatterns) {
      if (pattern.test(name)) {
        return {
          base: base,
          confidence: 'low',
          method: 'prefix-pattern-fallback',
          detail: `matched pattern for ${base}`
        };
      }
    }
  }

  // Unknown
  return {
    base: char || null,
    confidence: 'unknown',
    method: 'fallback',
    detail: `Could not parse: ${entityName}`
  };
}

function looksLikeModifiers(str) {
  if (!str) return true;

  let remaining = str.toLowerCase();

  // Try to consume modifiers from the string
  let iterations = 0;
  while (remaining.length > 0 && iterations < 20) {
    iterations++;
    let matched = false;

    // Sort modifiers by length (longest first) to avoid partial matches
    const sortedMods = MODIFIERS.sort((a, b) => b.length - a.length);

    for (const mod of sortedMods) {
      if (remaining.startsWith(mod)) {
        remaining = remaining.slice(mod.length);
        matched = true;
        break;
      }
    }

    // Also allow single digits
    if (!matched && /^[0-9]/.test(remaining)) {
      remaining = remaining.slice(1);
      matched = true;
    }

    if (!matched) {
      // Check if remaining looks like a partial modifier or garbage
      // Allow if it's just a few characters that could be abbreviations
      if (remaining.length <= 3) {
        return true; // Be lenient with short remainders
      }
      return false;
    }
  }

  return remaining.length === 0;
}

function main() {
  // Load entities
  const entitiesPath = join(__dirname, '../static/entities/menota.json');
  const entitiesData = JSON.parse(readFileSync(entitiesPath, 'utf-8'));

  const results = {
    high: [],
    medium: [],
    low: [],
    unknown: [],
    skip: [],
    combining: [],
    'xml-entity': [],
  };

  const mappings = {};

  for (const [name, data] of Object.entries(entitiesData.entities)) {
    const result = extractBaseLetter(name, data);

    const entry = {
      entity: name,
      char: data.char,
      unicode: data.unicode,
      description: data.description,
      ...result
    };

    if (result.method === 'combining-mark') {
      results.combining.push(entry);
      mappings[name] = '';
    } else if (result.confidence === 'skip') {
      results.skip.push(entry);
    } else if (result.confidence === 'unknown') {
      results.unknown.push(entry);
    } else {
      results[result.confidence].push(entry);
      if (result.base !== null) {
        mappings[name] = result.base;
      }
    }
  }

  // Output statistics
  console.log('\n=== Entity Mapping Statistics ===\n');
  console.log(`High confidence:   ${results.high.length}`);
  console.log(`Medium confidence: ${results.medium.length}`);
  console.log(`Low confidence:    ${results.low.length}`);
  console.log(`Unknown:           ${results.unknown.length}`);
  console.log(`Combining marks:   ${results.combining.length}`);
  console.log(`XML entities:      ${results['xml-entity'].length}`);
  console.log(`Skipped:           ${results.skip.length}`);
  console.log(`Total mapped:      ${Object.keys(mappings).length}`);

  // Output detailed results for review
  const outputPath = join(__dirname, '../static/normalizer/entity-mappings-draft.json');
  const output = {
    _meta: {
      generated: new Date().toISOString(),
      description: 'Draft entity to base letter mappings - REVIEW AND CORRECT',
      statistics: {
        high: results.high.length,
        medium: results.medium.length,
        low: results.low.length,
        unknown: results.unknown.length,
        combining: results.combining.length,
        xmlEntity: results['xml-entity'].length,
        skipped: results.skip.length,
      }
    },
    mappings: mappings,
    review: {
      medium_confidence: results.medium.slice(0, 100), // Sample for review
      low_confidence: results.low,
      unknown: results.unknown,
    }
  };

  writeFileSync(outputPath, JSON.stringify(output, null, 2));
  console.log(`\nDraft mappings written to: ${outputPath}`);

  // Also output a simpler mapping file for direct use
  const simpleMappingsPath = join(__dirname, '../static/normalizer/entity-base-letters.json');
  const simpleMappings = {
    version: '1.0',
    description: 'Entity name to base letter mappings for normalization',
    note: 'DRAFT - Review and correct before use',
    mappings: mappings
  };
  writeFileSync(simpleMappingsPath, JSON.stringify(simpleMappings, null, 2));
  console.log(`Simple mappings written to: ${simpleMappingsPath}`);

  // Show some examples
  console.log('\n=== Sample High Confidence Mappings ===\n');
  for (const entry of results.high.slice(0, 20)) {
    console.log(`  ${entry.entity.padEnd(25)} → '${entry.base}' (${entry.method})`);
  }

  console.log('\n=== Sample Unknown Entries (need review) ===\n');
  for (const entry of results.unknown.slice(0, 20)) {
    console.log(`  ${entry.entity.padEnd(25)} char='${entry.char}' - ${entry.description}`);
  }
}

main();

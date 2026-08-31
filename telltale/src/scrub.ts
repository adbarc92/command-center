/**
 * Ingest-side PII redaction (spec §4.3).
 *
 * Recorded operator decision: issues land in each project's own repo, including
 * public ones. That exposure was raised and knowingly accepted; this module is
 * the mitigation, not a guarantee — it misses obfuscated forms like
 * "alex at example dot com".
 *
 * Redaction is LOSSY AND ONE-WAY BY DESIGN. The original is never stored
 * anywhere: a store of unredacted originals would recreate the hazard.
 */

const EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g

// E.164 and NANP shapes. Requires a separator or a leading +, so a bare run of
// digits is left to the DIGIT_RUN rule (body only) and version strings survive.
const PHONE = /(?:\+\d{1,3}[\s.-]?)?(?:\(\d{3}\)|\d{3})[\s.-]\d{3}[\s.-]\d{4}\b/g

// 12+ consecutive digits: card and account numbers. BODY ONLY.
const DIGIT_RUN = /\b\d{12,}\b/g

export function scrubTitle(s: string): string {
  return s.replace(EMAIL, '[redacted:email]').replace(PHONE, '[redacted:phone]')
}

export function scrubBody(s: string): string {
  return scrubTitle(s).replace(DIGIT_RUN, '[redacted:number]')
}

// Reading the links an Entry hands to the app (see the frame script in entry_protocol.rs).
// Everything here comes from untrusted Entry content, so each function returns null for
// anything it does not fully understand.

/** Longer than any headword or resource path a real Dictionary uses. */
const MAX_LENGTH = 1024;

function decoded(text: string): string {
  try {
    return decodeURIComponent(text);
  } catch {
    return text; // a literal "%" that is not an escape
  }
}

/** The headword an `entry://` link points to, or null for anything else. */
export function linkedHeadword(href: string): string | null {
  const match = /^entry:\/\//i.exec(href);
  if (!match || href.length > MAX_LENGTH) return null;
  const [target = ""] = href.slice(match[0].length).split("#");
  const headword = decoded(target).trim();
  return headword === "" ? null : headword;
}

/**
 * The URL of a sound the Entry at `entrySrc` offers, given as a `sound://` link or as the
 * bare path inside the Dictionary. The URL is always beneath the Entry's own Dictionary
 * prefix: any path that could climb out of it is refused.
 */
export function soundUrl(entrySrc: string, href: string): string | null {
  if (href.length > MAX_LENGTH) return null;
  const scheme = /^([a-z][a-z0-9+.-]*):\/\//i.exec(href);
  if (scheme && scheme[1]?.toLowerCase() !== "sound") return null;
  const path = (scheme ? href.slice(scheme[0].length) : href).replace(/^[/\\]+/, "");
  const segments = path.split(/[/\\]/).map(decoded);
  if (segments.some((segment) => segment === "" || segment === "." || segment === ".." || /[/\\]/.test(segment))) return null;
  const base = new URL(".", entrySrc).href;
  const url = new URL(segments.map(encodeURIComponent).join("/"), base).href;
  return url.startsWith(base) ? url : null;
}

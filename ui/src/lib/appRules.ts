// Validation for the executable names in Arrange > App rules.

export const MAX_EXE_LEN = 128;

// Control characters, bidirectional overrides/isolates/marks and zero-width
// characters: they make a name look like something it is not.
// eslint-disable-next-line no-control-regex
const HIDDEN = /[\u0000-\u001f\u007f-\u009f\u061c\u200b-\u200f\u202a-\u202e\u2060-\u2069\ufeff]/;

/** Why `raw` is not an acceptable exe name, or null when it is. Trims first. */
export function exeNameError(raw: string, existing: readonly string[] = []): string | null {
  // Checked before trimming: trim() would quietly drop some of them (U+FEFF).
  if (HIDDEN.test(raw)) return 'The name contains hidden or control characters.';
  const name = raw.trim();
  if (name === '') return 'Type the name of the program, for example Notepad.exe.';
  if (/[\\/:]/.test(name)) return 'Use the file name only, without a folder (Notepad.exe, not C:\\…\\Notepad.exe).';
  if (name.length > MAX_EXE_LEN) return `Use at most ${MAX_EXE_LEN} characters.`;
  if (!/\.exe$/i.test(name) || name.length <= 4) return 'The name must end with .exe.';
  if (existing.some((e) => e.toLowerCase() === name.toLowerCase())) return 'There is already a rule for this program.';
  return null;
}

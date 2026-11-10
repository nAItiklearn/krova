import { describe, expect, it } from 'vitest';
import { MAX_EXE_LEN, exeNameError } from './appRules';

describe('exeNameError', () => {
  it('accepts plain exe names', () => {
    expect(exeNameError('Notepad.exe')).toBeNull();
    expect(exeNameError('  code.EXE ')).toBeNull();
    expect(exeNameError('a'.repeat(MAX_EXE_LEN - 4) + '.exe')).toBeNull();
  });

  it('rejects empty, non-exe and too long names', () => {
    expect(exeNameError('')).not.toBeNull();
    expect(exeNameError('   ')).not.toBeNull();
    expect(exeNameError('notepad')).not.toBeNull();
    expect(exeNameError('.exe')).not.toBeNull();
    expect(exeNameError('a'.repeat(MAX_EXE_LEN - 3) + '.exe')).not.toBeNull();
  });

  it('rejects paths', () => {
    expect(exeNameError('C:\\Windows\\notepad.exe')).not.toBeNull();
    expect(exeNameError('bin/app.exe')).not.toBeNull();
    expect(exeNameError('C:app.exe')).not.toBeNull();
  });

  it('rejects control, bidi and zero-width characters', () => {
    expect(exeNameError('a\u0007b.exe')).not.toBeNull();
    expect(exeNameError('note\u202Eexe.pad')).not.toBeNull();
    expect(exeNameError('note\u202Epad.exe')).not.toBeNull();
    expect(exeNameError('note\u200Bpad.exe')).not.toBeNull();
    expect(exeNameError('\uFEFFnotepad.exe')).not.toBeNull();
    expect(exeNameError('note\u2066pad.exe')).not.toBeNull();
  });

  it('rejects duplicates case-insensitively', () => {
    expect(exeNameError('notepad.exe', ['Notepad.exe'])).not.toBeNull();
    expect(exeNameError('code.exe', ['Notepad.exe'])).toBeNull();
  });
});

import { DOWNLOADS_ENABLED, MAC_RELEASED, RELEASE_PUBLISHED } from '../site';

/**
 * FAQ content. Answers use the inline markup from src/lib/inline.ts.
 * Keep each answer short and checkable against the app or the docs.
 */

export interface FaqItem {
  q: string;
  a: string;
}

export interface FaqGroup {
  title: string;
  items: readonly FaqItem[];
}

const whenAvailable = DOWNLOADS_ENABLED
  ? MAC_RELEASED
    ? 'Now. The [[/download/|Download page]] has the Windows installer and the macOS disk image.'
    : 'Now, for Windows: the [[/download/|Download page]] has the installer. macOS support is coming once it has been tested on real hardware.'
  : RELEASE_PUBLISHED
    ? 'The release is published, but the repository is not public yet, so there is no download link on this site. The [[/download/|Download page]] explains what to expect.'
    : 'The first release is not published yet. The [[/download/|Download page]] says what each build will need and how to get past the unsigned-installer warnings, so you can decide before it exists.';

export const faq: readonly FaqGroup[] = [
  {
    title: 'The app',
    items: [
      {
        q: 'How do I snap a window?',
        a: 'Start dragging the window and hold Shift. The zones of the monitor under your cursor appear, and the one you are over lights up. Release the mouse button and the window fills that zone. Release Shift first, or press Esc, to cancel. See [[/guides/first-snap/|Your first snap]].',
      },
      {
        q: 'Can I snap with a keyboard shortcut?',
        a: 'Yes, on Windows. In the 0.2 beta, Ctrl+Alt with an arrow key moves the focused window to the next zone that way (and on to the next monitor), Ctrl+Alt+1 to 9 sends it to that zone, and adding Shift swaps it with its neighbour or applies a layout. Every shortcut can be changed. In v0.1.0 the keys are Ctrl+Alt+Shift with Left, Right, Up or 1 to 9. There are no keyboard shortcuts on macOS yet. See [[/guides/keyboard-shortcuts/|Keyboard shortcuts]].',
      },
      {
        q: 'A shortcut does nothing. Why?',
        a: 'Windows, or another app, may already own that key combination: some code editors use Ctrl+Alt+Left and Right, and some older Intel graphics drivers use Ctrl+Alt+arrows to rotate the screen. In the 0.2 beta, the Shortcuts page marks a refused shortcut and suggests free keys; in v0.1.0, Settings, Keyboard shortcuts lists them and lets you pick other modifier keys. Shortcuts also do nothing for the desktop, the taskbar, minimized windows, administrator windows and Krova’s own windows.',
      },
      {
        q: 'Do windows go back to their zones when I plug a monitor back in?',
        a: 'On Windows, yes: windows Krova snapped to a monitor return to their zones a moment after that monitor comes back. This is kept in memory only (no window titles are stored), a window you drag yourself is forgotten, and quitting Krova clears it. It has been tested with a simulated monitor change; a real unplug and replug has not been tested yet. Windows 11 has its own setting that restores window positions: Krova leaves a window that is already in its zone and moves one that is not. There is no window memory on macOS yet.',
      },
      {
        q: 'What are linked dividers?',
        a: 'On Windows, after you snap windows side by side, drag the edge of one of them and let go: the snapped windows that share that edge follow. It is early. The part that moves the neighbours has been tested with synthetic events, but we have not confirmed that Windows reports a real mouse drag to Krova, so it may not work on your PC. You can switch it off under Settings, Windows.',
      },
      {
        q: 'Can I share a layout?',
        a: 'Yes. In the layout editor, Export layout shows the layout as plain text you can copy, and Import layout adds pasted text to a monitor as a new layout. Zones are saved as proportions of the screen, so the same text works on another monitor or computer, and should work on both Windows and macOS (nobody has tried it on a Mac yet). It is at most 64 KiB and must be in Krova’s exact format. See [[/guides/layout-editor/#share|the layout editor guide]].',
      },
      {
        q: 'Can I leave space between zones?',
        a: 'Yes: Settings, Windows, Space between zones, from 0 to 32 px (0 by default). It applies to zone snaps, the keyboard shortcuts, restored windows and the preview you see while dragging. Edge snapping ignores it.',
      },
      {
        q: 'Does it replace Windows Snap, or clash with it?',
        a: 'Krova does not turn Windows’ own snapping off. Its zones only appear while you hold its key, so the two stay out of each other’s way. Its keyboard shortcuts use Ctrl+Alt (Ctrl+Alt+Shift in v0.1.0), not the Windows key, so Win+Arrow keeps working. Krova’s edge snapping is different: if you switch it on, it works without a key and acts on the same screen edges as Windows’ own snapping. The two have not been tested together, so leave one of them off.',
      },
      {
        q: 'Which windows won’t it move?',
        a: 'Minimized windows, and (on Windows) windows of apps running as administrator. Dragging also skips fullscreen windows. The keyboard shortcuts leave the desktop, the taskbar and Krova’s own windows alone, and a maximized window is restored before it is moved. Some apps also refuse to go below a minimum size, so the window may not fill the zone exactly. More in [[/support/|Troubleshooting]].',
      },
      {
        q: 'Can I use a different key than Shift?',
        a: 'Yes. The key you hold while dragging can be Shift, Ctrl, Alt, or the Windows key (Command on macOS): Settings, Activation modifier. In the 0.2 beta it is under Arrange, Snapping, and every keyboard shortcut can be recorded again under Shortcuts. In v0.1.0 the shortcuts have one setting, Settings, Keyboard shortcuts, Modifier keys.',
      },
      {
        q: 'What happens when I close Krova’s window?',
        a: 'The window is closed for good, and so is the embedded browser that draws it, so Krova waits in the tray using very little memory. In one measurement during development the whole app was about 6 MB of private memory in one process with no window open, against about 200 MB across seven processes with the editor open (one run on one PC; the [[/performance/|Performance page]] shows published measurements). Reopen the window from the tray menu (Edit layouts or Settings) or by starting Krova again. Quit Krova stops it.',
      },
      {
        q: 'Is there a Linux version?',
        a: 'No. Krova supports Windows 10 and 11. A macOS version comes once it has been tested on a real Mac.',
      },
    ],
  },
  {
    title: 'Privacy',
    items: [
      {
        q: 'Does Krova connect to the internet?',
        a: 'No. By design it makes no network calls: no sign-in, no statistics, no crash reports, no update checks. The Windows installer may download Microsoft’s WebView2 runtime if it is missing. Details on the [[/privacy/|Privacy page]].',
      },
      {
        q: 'Can it see what is in my windows?',
        a: 'No. Krova looks at the position and size of the window you are dragging or moving with a shortcut, whether that window runs as administrator, and which window has the focus when you press a shortcut. On Windows it also remembers, in memory only, which window handle sits in which zone so it can put windows back. In the 0.2 beta it also reads the file name of a window’s program, for app rules and stack tabs, and a workspace you save stores program paths. It never reads window titles or contents.',
      },
      {
        q: 'Does Krova record what I type?',
        a: 'No. On Windows it registers only its own shortcut combinations with the system, which tells Krova when one of them is pressed; it does not use a keyboard hook, so Krova is not told about any other key. While you drag, it checks whether the activation key and Escape are held, nothing more. More on the [[/privacy/|Privacy page]].',
      },
      {
        q: 'What does it store, and where?',
        a: 'Settings and layouts live in one file, `config.json`, in your user application-data folder (`%APPDATA%\\dev.krova.desktop` on Windows). The remembered window positions are never written there. The settings window’s web view keeps a separate cache folder. Both are listed in [[/guides/install/|the install guide]], with how to remove them. A layout you export is plain text that you copy yourself; Krova writes no file for it.',
      },
      {
        q: 'Why does macOS ask for Accessibility access?',
        a: 'macOS only lets an app move and resize other apps’ windows once you allow it under System Settings, Privacy & Security, Accessibility. It may also ask for Input Monitoring so Krova can notice a drag. Without Accessibility, snapping cannot work.',
      },
    ],
  },
  {
    title: 'Download and install',
    items: [
      {
        q: 'When can I download it?',
        a: whenAvailable,
      },
      {
        q: 'Why does Windows say “Windows protected your PC”?',
        a: 'The installer is not code-signed, so SmartScreen does not recognise it. Choose More info, then Run anyway. Step by step in [[/guides/install/|the install guide]].',
      },
      {
        q: 'Has the macOS build been tested?',
        a: 'No. It has never been run on a real Mac, and it is not notarised. Treat it as experimental. It also has no keyboard shortcuts and no window memory yet. The Windows build has had more attention, but it is also new.',
      },
      {
        q: 'Is it free?',
        a: 'Yes. Krova is released under the MIT license: you may use, copy, modify and share it. Nothing is sold, so there is no purchase, subscription or account. See [[/license/|License and terms]].',
      },
      {
        q: 'How do I uninstall it and remove my data?',
        a: 'Uninstall from Windows Settings, Apps, and tick “Delete the application data”. The [[/guides/install/|install guide]] lists every file and registry entry Krova can leave behind, for Windows and macOS.',
      },
    ],
  },
];

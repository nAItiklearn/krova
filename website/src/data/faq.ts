import { DOWNLOADS_ENABLED, RELEASE_PUBLISHED } from '../site';

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
  ? 'Now. The [[/download/|Download page]] has the Windows installer and the macOS disk image.'
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
        a: 'Not in v0.1. Global shortcuts for snapping are an idea for v0.2, not a promise. Today you snap by dragging.',
      },
      {
        q: 'Does it replace Windows Snap, or clash with it?',
        a: 'Krova does not turn Windows’ own snapping off. Its zones only appear while you hold its key, so the two stay out of each other’s way. Krova’s edge snapping is different: if you switch it on, it works without a key and acts on the same screen edges as Windows’ own snapping. The two have not been tested together, so leave one of them off.',
      },
      {
        q: 'Which windows won’t it move?',
        a: 'Minimized windows, fullscreen windows, and (on Windows) windows of apps running as administrator. Some apps also refuse to go below a minimum size, so the window may not fill the zone exactly. More in [[/support/|Troubleshooting]].',
      },
      {
        q: 'Can I use a different key than Shift?',
        a: 'Yes: Shift, Ctrl, Alt, or the Windows key (Command on macOS). Change it under Settings, Activation modifier.',
      },
      {
        q: 'Is there a Linux version?',
        a: 'No. Krova supports Windows 10/11 and macOS 13 or later.',
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
        a: 'Krova looks at the position and size of the window you are dragging, and whether that window runs as administrator. It does not read window titles or contents, and none of it is stored.',
      },
      {
        q: 'What does it store, and where?',
        a: 'Settings and layouts live in one file, `config.json`, in your user application-data folder (`%APPDATA%\\dev.krova.desktop` on Windows). The settings window’s web view keeps a separate cache folder. Both are listed in [[/guides/install/|the install guide]], with how to remove them.',
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
        a: 'No. It has never been run on a real Mac, and it is not notarised. Treat it as experimental. The Windows build has had more attention, but it is also new.',
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

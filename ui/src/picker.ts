// Entry of the hover zone picker window (picker.html): a small frameless window
// the backend shows near the cursor while the snap key is held on a title bar.
import { mount } from 'svelte';
import '@fontsource-variable/inter/wght.css';
import './app.css';
import HoverPicker from './components/HoverPicker.svelte';
import { demoControls } from './lib/backend';

if (demoControls) {
  Object.assign(window, { __krovaDemo: demoControls });
}

const target = document.getElementById('picker');
if (!target) throw new Error('#picker not found');

export default mount(HoverPicker, { target });

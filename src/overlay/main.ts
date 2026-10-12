import { mount } from 'svelte';
import Overlay from './Overlay.svelte';
import '../app.css';

const app = mount(Overlay, { target: document.getElementById('app')! });

export default app;

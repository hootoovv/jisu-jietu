import { mount } from 'svelte';
import Settings from './Settings.svelte';
import '../app.css';

const app = mount(Settings, { target: document.getElementById('app')! });

export default app;

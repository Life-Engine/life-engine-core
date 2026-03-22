class MyPlugin extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'closed' });
  }

  connectedCallback() {
    const shell = this.__shellAPI;
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; padding: 1rem; }
        h1 { font-size: 1.5rem; margin: 0 0 1rem; }
      </style>
      <h1>${shell?.plugin?.id ?? 'My Plugin'}</h1>
      <p>Hello from a vanilla JS Life Engine plugin!</p>
    `;
  }
}

customElements.define('my-plugin', MyPlugin);

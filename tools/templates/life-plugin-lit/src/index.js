import { LitElement, html, css } from 'lit';

class MyLitPlugin extends LitElement {
  static styles = css`
    :host { display: block; padding: 1rem; }
    h1 { font-size: 1.5rem; margin: 0 0 1rem; }
  `;

  render() {
    return html`
      <h1>My Lit Plugin</h1>
      <p>Hello from a Lit-powered Life Engine plugin!</p>
    `;
  }
}

customElements.define('my-lit-plugin', MyLitPlugin);

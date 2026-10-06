import { LitElement, css, html } from 'lit';
import { customElement, state } from 'lit/decorators.js';

const steps = [
  ['Map the room', 'Find stable floors, walls, corners, and other surfaces that can become spatial references.'],
  ['Place anchors', 'Add nearby phones or ranging anchors. Scanner evaluates geometry and recommends better placement.'],
  ['Establish spatial lock', 'Walk the space while Scanner fuses visual tracking, motion, depth, and ranging constraints.'],
  ['Confirm structure', 'Review high-confidence structural surfaces and explicitly lock the ones that should remain fixed.'],
  ['Scan the subject', 'Move around the object while geometry, coverage, texture, and pose confidence update in real time.'],
  ['Repair uncertainty', 'Tap an uncertain region in the live view and Scanner will guide you to the observation that adds the most information.']
] as const;

@customElement('onboarding-flow')
export class OnboardingFlow extends LitElement {
  @state() private index = 0;

  static styles = css`
    :host { display: block; }
    .panel {
      min-height: 17rem;
      display: grid;
      grid-template-rows: auto 1fr auto;
      gap: 1.25rem;
      padding: 1.5rem;
      border: 1px solid var(--line);
      border-radius: 1.5rem;
      background: var(--panel);
    }
    .count {
      color: var(--muted);
      font: 600 .78rem/1.2 var(--mono);
      letter-spacing: .08em;
      text-transform: uppercase;
    }
    h3 { margin: .4rem 0 .7rem; font-size: clamp(1.5rem, 4vw, 2.2rem); }
    p { color: var(--muted); max-width: 44rem; }
    nav { display: flex; gap: .65rem; flex-wrap: wrap; }
    button {
      border: 1px solid var(--line);
      background: var(--surface);
      color: var(--text);
      border-radius: 999px;
      padding: .7rem 1rem;
      font: inherit;
      cursor: pointer;
    }
    button.primary { background: var(--text); color: var(--bg); }
    button:focus-visible { outline: 3px solid var(--focus); outline-offset: 3px; }
  `;

  render() {
    const [title, body] = steps[this.index]!;
    return html`
      <section class="panel" aria-live="polite">
        <div class="count">Step ${this.index + 1} of ${steps.length}</div>
        <div>
          <h3>${title}</h3>
          <p>${body}</p>
        </div>
        <nav aria-label="Onboarding steps">
          <button ?disabled=${this.index === 0} @click=${() => this.index = Math.max(0, this.index - 1)}>Back</button>
          <button class="primary" @click=${() => this.index = (this.index + 1) % steps.length}>
            ${this.index === steps.length - 1 ? 'Start again' : 'Next'}
          </button>
        </nav>
      </section>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'onboarding-flow': OnboardingFlow;
  }
}

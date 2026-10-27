import { Container, Eyebrow } from "../ui";

import "./HeroSection.css";

/**
 * Hero section.
 *
 * Typography-led and single-column: the headline carries the page, with the
 * eyebrow, lead and status line aligned to one measure. No illustration, no
 * card, no divider, and no download button — there is no installer to link to,
 * so the release state is stated as plain text instead of an inert control.
 *
 * Copy is limited to what holds today: Nexora is a Windows desktop AI coding
 * assistant *designed* for a local-first workflow; local inference is the
 * intended path once a model is present, and the Windows download does not
 * exist yet.
 */
export default function HeroSection() {
  return (
    <section className="hero" aria-labelledby="hero-title">
      <Container>
        <div className="hero__inner">
          <Eyebrow className="hero__eyebrow">Local AI · Windows desktop</Eyebrow>

          <h1 className="hero__title" id="hero-title">
            <span className="hero__title-line">Your AI coding workspace.</span>{" "}
            <span className="hero__title-line">On your terms.</span>
          </h1>

          <p className="hero__lead">
            Nexora is a Windows desktop AI coding assistant built around local
            models — designed for a more private, locally run coding workflow once
            the model is downloaded.
          </p>

          <p className="hero__status">
            <span className="hero__status-dot" aria-hidden="true" />
            In active development. The Windows installer is not available yet.
          </p>
        </div>
      </Container>
    </section>
  );
}

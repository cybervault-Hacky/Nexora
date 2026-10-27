import { Link } from "react-router-dom";

import { FaqCategory, PageIntro } from "../components/sections";
import { Container, Section } from "../components/ui";
import { FAQ_CATEGORIES } from "../content/faq";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";

import "./FaqPage.css";


const questionCount = FAQ_CATEGORIES.reduce(
  (total, category) => total + category.items.length,
  0,
);

/**
 * FAQ page.
 *
 * Answers are limited to what the source and documentation support, and are
 * written to agree with the Download, Privacy, and Security pages. Questions are
 * native disclosures, so no script is involved in opening one.
 */
export default function FaqPage() {
  usePageMetadata(ROUTES.faq);

  return (
    <>
      <Section>
        <Container>
          <PageIntro
            eyebrow="FAQ"
            title="Frequently asked questions."
            lead="Short answers, checked against the project's source. Where something is not built, not known, or not verified, the answer says so instead of guessing."
          />

          <p className="faq__note">
            {questionCount} questions across {FAQ_CATEGORIES.length} categories.
            Each one opens in place; the answer text is part of the page even
            while it is collapsed, so browser search and screen readers can still
            reach it.
          </p>
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          {FAQ_CATEGORIES.map((category) => (
            <FaqCategory category={category} key={category.id} />
          ))}

          <p className="faq__closing">
            If a question here is not answered, the{" "}
            <Link to={ROUTES.support}>support page</Link> explains what you can
            check yourself and what to record if something goes wrong. The
            underlying detail lives on the{" "}
            <Link to={ROUTES.privacy}>privacy</Link> and{" "}
            <Link to={ROUTES.security}>security</Link> pages.
          </p>
        </Container>
      </Section>
    </>
  );
}

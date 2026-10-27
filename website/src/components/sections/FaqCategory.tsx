import { Link } from "react-router-dom";

import type { FaqCategory as FaqCategoryData } from "../../content/faq";

import "./FaqCategory.css";

export interface FaqCategoryProps {
  category: FaqCategoryData;
}

/**
 * A group of questions, each rendered as a native disclosure.
 *
 * Uses `<details>`/`<summary>` rather than a scripted accordion: the browser
 * already gives every question keyboard operation (Enter and Space on the
 * summary, focus in document order, and the `open` state exposed to assistive
 * technology), and with no state to hydrate the answers are present in the HTML
 * even when collapsed. Nothing here is a heading wrapper, so a collapsed list
 * still reads as a list of questions.
 *
 * The marker is drawn with CSS borders and carries `aria-hidden`, because the
 * native disclosure state is already announced — a decorative plus must not be
 * read out.
 */
export default function FaqCategory({ category }: FaqCategoryProps) {
  return (
    <div className="faq-category">
      <h2 className="faq-category__title" id={category.id}>
        {category.title}
      </h2>
      {category.intro ? (
        <p className="faq-category__intro">{category.intro}</p>
      ) : null}

      <div className="faq-category__items">
        {category.items.map((item) => (
          <details className="faq-item" id={item.id} key={item.id}>
            <summary className="faq-item__summary">
              <span className="faq-item__question">{item.question}</span>
              <span className="faq-item__marker" aria-hidden="true" />
            </summary>
            <div className="faq-item__body">
              <p className="faq-item__answer">{item.answer}</p>
              {item.links?.length ? (
                <p className="faq-item__links">
                  {item.links.map((link, index) => (
                    <span key={link.to}>
                      {index > 0 ? <span aria-hidden="true"> · </span> : null}
                      <Link to={link.to}>{link.label}</Link>
                    </span>
                  ))}
                </p>
              ) : null}
            </div>
          </details>
        ))}
      </div>
    </div>
  );
}

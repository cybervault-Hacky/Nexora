import type { AnchorHTMLAttributes, ButtonHTMLAttributes } from "react";
import { Link } from "react-router-dom";

import "./Button.css";

export type ButtonVariant = "primary" | "secondary";
export type ButtonSize = "sm" | "md" | "lg";

interface CommonButtonProps {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Stretches the button to the width of its container. */
  block?: boolean;
}

type ButtonAsButton = CommonButtonProps &
  ButtonHTMLAttributes<HTMLButtonElement> & { href?: undefined; to?: undefined };

type ButtonAsAnchor = CommonButtonProps &
  AnchorHTMLAttributes<HTMLAnchorElement> & { href: string; to?: undefined };

interface ButtonAsRouteLink
  extends CommonButtonProps,
    Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> {
  /** Internal route. Rendered as a client-side link, not a full reload. */
  to: string;
  href?: undefined;
}

export type ButtonProps = ButtonAsButton | ButtonAsAnchor | ButtonAsRouteLink;

function classNames(
  variant: ButtonVariant,
  size: ButtonSize,
  block: boolean,
  className: string | undefined,
): string {
  return [
    "btn",
    `btn--${variant}`,
    `btn--${size}`,
    block ? "btn--block" : undefined,
    className,
  ]
    .filter(Boolean)
    .join(" ");
}

/**
 * Button primitive.
 *
 * Renders:
 * - a router `<Link>` when `to` is supplied (in-site navigation, no reload),
 * - an `<a>` when `href` is supplied (external or file destinations),
 * - a `<button>` otherwise.
 *
 * Navigation is always a link and actions are always a button. Disabled
 * styling is native on `<button>`; links intentionally have no disabled state.
 */
export default function Button(props: ButtonProps) {
  const {
    variant = "primary",
    size = "md",
    block = false,
    className,
    ...rest
  } = props;

  const classes = classNames(variant, size, block, className);

  if (typeof rest.to === "string") {
    const { to, ...linkProps } = rest as ButtonAsRouteLink;
    return <Link to={to} className={classes} {...linkProps} />;
  }

  if (typeof rest.href === "string") {
    const { href, ...anchorProps } = rest as ButtonAsAnchor;
    return <a href={href} className={classes} {...anchorProps} />;
  }

  const buttonProps = rest as ButtonAsButton;
  return (
    <button
      type={buttonProps.type ?? "button"}
      className={classes}
      {...buttonProps}
    />
  );
}

export const ButtonSearch = ({ type = "button", ...props }) => {
  return (
    <button
      type={type}
      aria-label="Search"
      className="button-search"
      {...props}
    >
      <span className="button-search__icon" aria-hidden="true" />
    </button>
  );
};

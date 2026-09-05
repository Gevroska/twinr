import { ParentComponent, createSignal, createUniqueId } from "solid-js";

const WatchDetails: ParentComponent = (props) => {
  const [expanded, setExpanded] = createSignal(false);
  const contentId = createUniqueId();

  return (
    <div class="watch-details">
      <button
        class="watch-details-toggle"
        aria-expanded={expanded()}
        aria-controls={contentId}
        onclick={() => setExpanded((value) => !value)}
      >
        <span>Video details</span>
        <span aria-hidden="true">{expanded() ? "−" : "+"}</span>
      </button>
      <div id={contentId} class="watch-details-content" data-expanded={expanded()}>
        {props.children}
      </div>
    </div>
  );
};

export default WatchDetails;

import iconSrc from "../../assets/icon-32.png";

const HandyHand = ({
  width,
  height,
}: {
  width?: number | string;
  height?: number | string;
}) => (
  <img
    src={iconSrc}
    width={width || 24}
    height={height || 24}
    alt="Meetdy"
    style={{ objectFit: "contain" }}
  />
);

export default HandyHand;

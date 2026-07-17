import {
    Container,
    createImageResource,
    Image,
    view,
} from "builtin:tur/std";

const pngBytes = new Uint8Array([
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0,
    0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120,
    218, 99, 252, 255, 159, 161, 30, 0, 7, 130, 2, 127, 61, 200, 72, 239, 0, 0,
    0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
]);
const resource = createImageResource(pngBytes);

export default view(() =>
    Container({
        width: 200,
        height: 100,
        children: [
            Image({
                resourceId: resource,
                width: 200,
                height: 100,
                fit: 0,
            }),
        ],
    }),
);

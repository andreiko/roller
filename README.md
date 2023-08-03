# Roller

Roller is a toy that simulates throwing a polyhedral game dice of your choice multiple times.

For example, if you need to make an attack roll of 4d20 (throw 20-sided die 4 times and add up the results),
turn one of the two settings knobs on the side to choose 4 and the other knob to choose 20 until the display is showing
"4d20", then shake or roll the sphere on the table. Once you stop shaking it, or it finds balance after the roll, it'll
display the result for you!

## Demo

TODO: upload.

## Electrical design

To fit into the smallest possible enclosure, electronics are split between two boards connected by a ribbon.

### Main board schematic

![Schematic](docs/main-schematic.png)

### Main board design

![Design](docs/main-design.png)

### Main board result

| ![Top](docs/main-top.jpg)          | ![Bottom](docs/main-bottom.jpg)          |
|------------------------------------|------------------------------------------|
| ![Top](docs/main-soldered-top.jpg) | ![Bottom](docs/main-soldered-bottom.jpg) |

### Display board schematic

![Schematic](docs/display-schematic.png)

### Display board design

![Design](docs/display-design.png)

### Display board result

| ![Top](docs/display-top.jpg)          | ![Bottom](docs/display-bottom.jpg)          |
|---------------------------------------|---------------------------------------------|
| ![Top](docs/display-soldered-top.jpg) | ![Bottom](docs/display-soldered-bottom.jpg) |

## Mechanical design

The boards are contained in a spherical enclosing that has openings for the display, settings knobs and the on/off
switch:

| ![Display and Knobs](docs/case-knobs.png) | ![Switch](docs/case-switch.png) |
|-------------------------------------------|---------------------------------|

The enclosing consists of three 3D-printed pieces: 2 half-spheres and a battery door:
![Disassembled](docs/case-disassembled.png)

## Firmware

The logic of the device is implemented as a Rust program for the AVR architecture.

The program has 3 main states:

| State      | Description                                                                                                                                                          | Power consumption |
|------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------|
| Displaying | Initial state. Displays settings (6d6, 5d20) or a roll result (number). Enters this state after a roll or when settings knobs are turned.                            | 10-22 mA          |
| Rolling    | Displays rolling animation, collects entropy from the accelerometer. Enters this state when the device is shaken or rolled. Exits when no more movement is detected. | 10 mA             |
| Sleeping   | Display is off and the internal timer's frequency is reduced to save power. Enters this state after being idle for 30s.                                              | 0.2 mA            |

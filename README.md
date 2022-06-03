<h1 align="center">💣 Squad Mortar Helper (SMH)</h1>

<p align="center">SMH is a computer vision toy project aimed at automating mortar calculations in the game <a href="https://joinsquad.com/">Squad</a></p>

<h3 align="center"><a href="https://github.com/WilliamVenner/squad-mortar-helper/releases/latest">Download</a></h3>

<br/>

https://user-images.githubusercontent.com/14863743/168590102-8a520077-55f6-4b5d-ac34-8ffdb2b1fa23.mp4

<br/>

![image](https://user-images.githubusercontent.com/14863743/170140566-62c5a34a-1b12-4c5c-b595-f7643368f90b.png)

<br/>

# Requirements

* x86-64 CPU
* Windows or Linux
* (Windows) Visual C++ Redistributable Runtime 2015 (Squad installs this anyway)

# Features

* Automatically detects your Squad Leader's markers and calculates distance, altitude difference and milliradians
* Start a local web server with a couple clicks for interacting with SMH using your mobile phone or the Steam browser
* Rip heightmaps from the game for improved mortar calculation accuracy
* Supports ripping heightmaps from installed mods
* Choose between using your CPU or GPU for computer vision
* Hold left click on the map to draw custom markers
* Hold right click on the map for a quick range-finder
* Use scroll wheel and middle mouse button to pan and zoom the map

# FAQ

## How does it work?

SMH uses computer vision and OCR (optical character recognition) to extract information from your in-game map, such as the map markers you or your SL has placed and the scales in the bottom-right of the map which map pixels to meters.

Using this information, SMH can tell you roughly how many meters away a marker is, and an estimate of the milliradians to configure your mortar to.

SMH can also be used as a range-finder and you can place your own markers by drawing on the map.

## Can I get banned by EAC for using this?

No, the program does not attach to or read memory from the Squad game process.

SMH only takes screenshots of the game window using normal operating system APIs, and is akin to a normal screenshotting tool such as ShareX, Lightshot, Gyazo etc.

The program is completely open source and you can read through the code yourself!

## I only have one monitor, how am I supposed to see the program?

Fear not - the program is capable of starting a minimal webserver on your local network which you can use to see the program on your mobile device or Steam browser!

## Tips

The algorithms are not perfect – Squad's map can be quite noisy, and some map blips overlap information SMH analyzes. Here are some tips to help it out.

* **SELECT A HEIGHTMAP!** Heightmaps can be used to accurately calculate altitude difference _and_ distance in meters
* **Don't** use software such as f.lux that affect the colours of your screen (some OS-level filters like the Windows "Night light" work fine though)
* **Don't** zoom in to the in-game map. SMH needs the map to be fully zoomed out for heightmaps to line up properly. However, you can pan and zoom SMH's map - see the Features section.
* **TURN OFF** "Toggle Viewing Roles as Player Icons" in the map sidebar for improved line segment detection
* Keep "Map Icon Scaling" set to 0.7 in the map sidebar for improved marker detection
* Try to stay away from blips such as vehicles, HABs, etc. as they overlap squad leader markers, turn them off in the map sidebar if necessary
* Listen to your team mates and squad for feedback on how accurate your hits are, correct if needed
* If SMH isn't picking something up, you can draw your own markers on the map yourself!

# Hardware Acceleration (GPU processing)

If you have a NVIDIA GPU, SMH supports using its CUDA cores for extremely fast computer vision processing.

This can be enabled or disabled in `Settings > Hardware Acceleration`, as you may prefer to offload work to the CPU rather than the GPU (which is trying to render Squad!)

I have made my best efforts to optimise the code to reduce resource usage in order to not starve Squad of the computing power it needs, but please bear in mind that computer vision is not an easy process. Your computer is going to have to work to make these calculations, so this program may not be be appropriate for lower end rigs. Do give it a try though, especially if you have a NVIDIA GPU, as the performance may surprise you. If you have a CPU with a lot of cores, it should also be very capable with hardware acceleration disabled.

# Heightmaps

Heightmaps are ripped directly from the game files. You can select a heightmap to use in `Heightmaps > Select`. You can also export heightmaps as a grayscale PNG file, and see information such as the heightmap's scale, minimap bounds, texture corners, etc.

Please feel free to use the heightmap ripper for your own mortar calculator projects. I hope that it is useful!

# Building

If you would like to compile this program yourself, you can follow [these instructions](BUILDING.md).

Compiling this program yourself may be beneficial as targetting your exact CPU can enable further optimisations.

# Credits

**[Badger](https://github.com/Badger9)** – Line Segment Detection Algorithm (CPU)

**[Tesseract OCR](https://github.com/tesseract-ocr/tesseract)** – Optical Character Recognition

**[The Rust CUDA Project](https://github.com/Rust-GPU/Rust-CUDA)** – NVIDIA CUDA for Rust

**[CUE4Parse](https://github.com/FabianFG/CUE4Parse)** – UE4 File Format Parser for ripping heightmaps

**[Dear Imgui](https://github.com/ocornut/imgui)** – User Interface Library

**[SquadMC Team](https://github.com/Endebert/squadmc-maps/wiki/How-to-add-new-maps-to-SquadMC)** – Heightmap Guide

Various Squad Mortar Calculators – Projectile Calculations

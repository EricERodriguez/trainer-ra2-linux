This is a simple trainer for Command & Conquer Red Alert 2, designed for
using on an UNIX or UNIX-like system. Distributed under the X11 license, see
file `COPYING` for details.

It requires:

* Unix shell (**sh(1)**),
* GNU debugger (**gdb(1)**)

Fully supporting Red Alert version 1.000 and version 1.006; partially
supporting Yuri's Revenge version 1.000.


|Script                         |Description                                     |
|-------------------------------|------------------------------------------------|
|`we-can-build-anywhere.sh`     |Allow placing structures outside base range     |
|`we-are-free.sh`               |Stop credit from decreasing                     |
|`we-are-watching-everywhere.sh`|Reveal full map as long as you own any structure|
|`our-engineer-fixed-radar.sh`  |Turn on the radar map unconditionally           |

### Usage

On Linux-based operating systems, simply run the selected shell script with
`auto` as command line argument, for example:
```sh
sh we-can-build-anywhere.sh auto
```

Finding the game process automatically may not work on other operating
systems, you will need to pass the PID manually instead of `auto` in such
case.

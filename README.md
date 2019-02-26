# Flux FOV

*Flux FOV* is is an experimental [field-of-vision][1] algorithm and tool for
roguelike games.  It is based on the idea of modelling the influx and outflux
of rays (of light, eyesight, radiation, etc.) at the level of an individual
grid cell.

As noted above this is **just an experiment**.  While seeming to work way
better than I anticipated it is still inferior to, for example, shadow casting
algorithms.

[1]: http://www.roguebasin.com/index.php?title=Category:FOV
     "Roguebasin -- Category: FOV"

## Try it out

```sh
$ cargo run --example simple
```

## License

Copyright (C) 2019 Matti Hänninen

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

This program is distributed in the hope that it will be useful, but WITHOUT
ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
details.

You should have received a copy of the GNU General Public License along with
this program.  If not, see <http://www.gnu.org/licenses/>.

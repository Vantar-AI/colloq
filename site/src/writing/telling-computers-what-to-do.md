# How we told computers what to do

The first program I wrote that mattered was a shell loop that renamed four hundred photos.
It took me an hour. Doing it by hand would have taken two. I did not care, because
something had changed: I had described the work instead of performing it.

Every generation of programmers has a version of that moment, and every generation has it
at a higher altitude than the last. Looking at where the altitude has moved says more
about the next ten years than most predictions do.

## Wires, then numbers

The first machines were told what to do by rewiring them. A program was a physical
arrangement. Changing it meant moving cables, and the people who did that work were not
called programmers, because the word did not exist yet.

Then came stored programs, and instructions became numbers in the same memory as the data.
That sounds like an implementation detail. It was the entire idea. Once an instruction is
just a number, a program can write a program, and everything that followed is a variation
on that fact.

The instructions were still numbers, though, and a human had to hold the meaning in their
head. If you have ever seen a photograph of a programmer from that era holding a listing
of octal, you have seen someone doing translation work by hand.

## Names instead of numbers

Assembly gave the numbers names. `ADD` instead of an opcode. A label instead of an
address. It looks like a small step and it was a huge one, because it introduced the idea
that the text you write is not the thing that runs. Something in between translates.

Once that gap existed, people started widening it.

FORTRAN let you write an expression that looked like the mathematics it came from. COBOL
let you write something that read like a sentence about a business. Both were argued about
in terms that will sound familiar: too slow, too far from the machine, fine for small jobs
but not for real work.

The arguments were correct at the time and irrelevant in the end. The translator improved
every year, and the human did not have to.

## Structures instead of jumps

By the 1970s the problem was not speaking to the machine. It was keeping a large program
in your head. Programs had grown past the size where one person could hold the whole
control flow, and the tool for control flow was the jump.

Structured programming was a rule about what you may not do. No arbitrary jumps. Loops and
branches with one way in and one way out. Functions with a signature you can read without
reading the body.

That pattern repeats often enough to be worth naming. A lot of progress in programming has
come from removing something. Jumps. Manual memory management. Shared mutable state.
Implicit conversions. In each case the argument against removal was the same: you are
taking away power. In each case the answer was the same: the power was being spent on
mistakes.

## Libraries, then frameworks, then services

The 1990s and 2000s moved the altitude again, in a different direction. Instead of better
instructions, we got larger prefabricated parts.

You stopped writing a sort. Then you stopped writing a hash table. Then you stopped
writing a web server, a session store, a template engine, a queue. The unit of reuse grew
from a function to a library to a framework to an entire service you rent.

This is the part of the history that is usually told as pure progress, and it was not. Each
step up moved work rather than removing it. Writing the sort was replaced by choosing the
library, tracking its versions, reading its release notes, and debugging the part of it
that does not fit your case. Renting the service replaced the code you did not want to
write with a contract you did not write either.

Ask anyone who has spent a week on a dependency upgrade what the framework saved them, and
then ask them what it cost. The honest answer is usually that it was still worth it, and
that the accounting is messier than the pitch.

## Prompts

Now a model writes the code. Not all of it, and not reliably, but enough that the shape of
the job has changed for a lot of people.

This looks like a bigger jump than the ones before it, and in one way it is: the input is
natural language, which is the interface humans have wanted since the beginning. In another
way it is exactly the same jump as the rest. The unit of instruction got bigger. You say
what you want at a higher level, and something in between translates.

What makes it feel different is that the translator is no longer deterministic. A compiler
given the same input produces the same output. A model does not, and it is confident when
it is wrong. So the historical pattern still holds, with one new term: as the unit of
instruction grows, the thing you must check grows with it.

That is the part I would pay attention to. Every step in this history made writing cheaper
and made verification more important. We optimised the first half of that sentence for
seventy years and mostly left the second half alone.

## What each step actually removed

Look at the sequence again with one question: what did each step stop humans from having
to know?

- Stored programs removed the wiring.
- Assembly removed the numeric address.
- High-level languages removed the register.
- Structured programming removed the arbitrary jump.
- Garbage collection removed the manual free.
- Managed services removed the machine.
- Models are removing the line of code.

None of those removals was free, and none of them removed the need to know whether the
result is correct. The check moved. It did not shrink.

Assembly moved the check to the assembler. Types moved part of it into the compiler. Tests
moved part of it into a suite you run. Each of those was an attempt to make correctness
something a machine could verify, because the code volume had grown past what a human
could verify by reading.

Code volume has now grown again, by a lot, and fast.

## The obvious next move

If writing is cheap and checking is expensive, then the valuable thing is whatever makes
checking mechanical.

That is not a prediction about models. It is an old lesson. Every time the volume of code
went up, the tools that survived were the ones that turned a human judgement into a machine
check. Static types survived because they catch a class of error without a human. Memory
safety survived because it removes a class of error a human cannot reliably catch. Tests
survived, with all their problems, because a test is a question the machine can ask again
tomorrow.

So when someone asks what programming looks like in ten years, I do not think the
interesting question is how the code gets written. That question has an obvious answer now,
and it keeps getting more obvious. The interesting question is what gets checked, and by
what.

My own bet is narrow and I will say it plainly, because I work on it: the hardest thing to
check today is whether two programs that talk to each other still agree, and that is
exactly the check that no amount of code generation fixes by itself. Generating both sides
of an agreement faster does not make them agree. It makes them disagree sooner.

But that is a separate essay. The point of this one is smaller. We have spent seventy years
raising the level at which we tell computers what to do, and every single time, the thing
that mattered afterwards was not the writing. It was knowing whether the result was right.

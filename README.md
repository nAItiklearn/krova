# Krova

An app to keep your windows dekstop organised,
made by me(and claude code)


## what is

Its free , offline with no sign up or cookies thingy on website - https://naitiklearn.github.io/krova/

it lets you organise , make custom zones , manage workspaces and much more . 
you can pretty much organise your tabs any how.

there are features like focus mode , switch to different tabs using keyboart shortcuts, krova handles the positioning automatically

## why I made this 

So I was doomscrollin x and saw someone created something like this to manage your mac workspaces  ofc available only for mac + paid . So I thought to bless windows users 

## AI usage
its made by claude code fully, but I still had to fix many things and debugg and test each feature myself , there are still some known bugs like , sometime it doesnt tile some tabs , it may work kinda slow if you have a potato laptop.
, ill fix all these in v2 , trust .

## tech stack
- Rust
- tauri 2
- vite
-TypeScript 

##  run locally
1. clone the repo
```
git clone https://github.com/nAItiklearn/krova.git
cd krova
```
2. install dependencies
```
npm ci --prefix ui
```
3. start application
```
npx -y @tauri-apps/cli@2 dev
```
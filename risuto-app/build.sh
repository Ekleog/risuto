#!/bin/sh

# Build the app
cd ../risuto-web
trunk build --public-url "/REMOVE-ME/"

# Clean leftovers
rm -rf ../risuto-app/www
mkdir ../risuto-app/www
touch ../risuto-app/www/.empty

# Copy app
cp -r dist/* ../risuto-app/www
cp -r vendor ../risuto-app/www

# Fixup path
sed -i -e 's_/REMOVE-ME/_./_g' ../risuto-app/www/index.html

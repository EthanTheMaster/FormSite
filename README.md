## What is this project?
A simple toy polling/survey website written in `Rust` using the `actix-web` web framework. 

## Features
* Supports free response questions and multiple choice questions
* Responses can be publicly or privately viewed
* Submissions can be exported to a `.csv` file  

## How to install
* Run `docker-compose up -d --build` inside the project directory
* After the services and containers are running, you will need to load the mysql database. _This is only for the setup as all the database information will be kept in a volume._
    * Import the `FormSite.sql` into the mariadb container: `docker cp FormSite.sql MARIADB_CONTAINER_HASH:/var/lib/mysql/FormSite.sql`
    * Enter the mariadb container to load in the database information: `docker exec -it MARIADB_CONTAINER_HASH bash`
    * Run this inside the container's shell: `mysql -uroot -p"MARIADB_PASSWORD" < /var/lib/mysql/FormSite.sql`
        * The default password is `password` so execute `mysql -uroot -ppassword < /var/lib/mysql/FormSite.sql`
    * The database should be set up and you are ready to go now!
        * You should find the website locally at `0.0.0.0:8080` 